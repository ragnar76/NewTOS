//! VDI raster operations — `vrt_cpyfm` (transparent mono→colour blit)
//! and `vro_cpyfm` (opaque copy). Spec: `docs/api/vdi/raster/_raster.md`.
//!
//! **MFDB pointer convention (NewTOS native).** The classic VDI packs a
//! 32-bit MFDB pointer into `CONTRL[7..8]` (src) / `CONTRL[9..10]`
//! (dst). Native AArch64 pointers are 64-bit, so NewTOS places the
//! source MFDB pointer in `CONTRL[7..10]` and the destination in
//! `CONTRL[11..14]` (four WORDs each). `fd_addr == NULL` means the
//! screen device.
//!
//! Scope: `vrt_cpyfm` with a 1-plane source (icons/sprites), all four
//! `MD_*` write modes (replace / transparent / XOR / erase); `vro_cpyfm`
//! for all four source/destination combinations with the full 16-entry
//! logic-op table — Screen↔Screen, **Screen→Memory** (AES `bb_save`),
//! **Memory→Screen** (`bb_restore`), and **Memory↔Memory**.
//!
//! **NewTOS memory-MFDB format (device form, `fd_stand = 0`).** A memory
//! MFDB (`fd_addr != 0`) is a **32-bpp chunky truecolor** buffer matching
//! the backend's framebuffer store: one `u32` (XRGB8888) per pixel. The
//! row stride is derived from `fd_wdwidth` (16-pixel words, so the row is
//! rounded up to a multiple of 16 px, mirroring EmuTOS `bb_set`'s word
//! alignment) — a memory copy uses the MFDB's *own* geometry, never the
//! screen stride. `fd_nplanes = 32` marks the 32-bpp depth; with it the
//! classic interleaved-stride identity `bytes/line = fd_wdwidth *
//! fd_nplanes / 8 * 16 = fd_wdwidth * 64` holds. This differs from the
//! m68k planar `bb_save` (which uses `fd_stand = TRUE`, plane-sequential):
//! our screen store is chunky, so we save/restore the raw device pixels
//! (`docs/api/vdi/raster/_raster.md`, "NewTOS memory-MFDB format";
//! `docs/COMPATIBILITY.md` — the *observable* save/restore contract is
//! reproduced, the internal buffer layout is ours). The buffer is
//! caller-owned (AES `bb_save` allocates it); the VDI only reads/writes
//! through `fd_addr`, always bounds-checked against `fd_w × fd_h`.

use super::backend::{ClipRect, MonoSrc, RasterBackend, Rect, WriteMode};
use super::vwk::{validate_color_index, Vwk};
use super::Vdipb;

/// The `S_ONLY` logic op (`INTIN[0] = 3`, `D' = S`, replace) — the op AES
/// `bb_save`/`bb_restore` use (EmuTOS `gemgsxif.c` `bb_set`).
const OP_REPLACE: u8 = 3;

/// Memory Form Definition Block (fixed ABI; native pointer in `addr`).
/// `stand`/`nplanes` are parsed for completeness (they describe the form
/// but the pixel walk only needs `addr`/`w`/`h`/`wdwidth`).
#[allow(dead_code)]
struct Mfdb {
    addr: u64, // 0 = screen
    w: i16,
    h: i16,
    wdwidth: i16,
    stand: i16,
    nplanes: i16,
}

impl Mfdb {
    /// Read an MFDB from its native pointer (0 ⇒ a null/screen MFDB).
    ///
    /// # Safety
    /// `ptr` must be 0 or a valid MFDB.
    unsafe fn from_ptr(ptr: u64) -> Self {
        if ptr == 0 {
            return Mfdb { addr: 0, w: 0, h: 0, wdwidth: 0, stand: 0, nplanes: 0 };
        }
        // SAFETY: caller guarantees a valid MFDB.
        unsafe {
            let p = ptr as *const u8;
            let addr = (p as *const u64).read_unaligned();
            let word = |off: usize| (p.add(off) as *const i16).read_unaligned();
            Mfdb {
                addr,
                w: word(8),
                h: word(10),
                wdwidth: word(12),
                stand: word(14),
                nplanes: word(16),
            }
        }
    }

    /// Build a NewTOS 32-bpp device-format memory MFDB over a caller-owned
    /// `u32` pixel buffer of `w × h` pixels (AES `bb_save`): `stand = 0`
    /// (device), `nplanes = 32` (32-bpp chunky), `wdwidth = (w + 15) / 16`.
    fn device_mem(addr: u64, w: i16, h: i16) -> Self {
        Mfdb { addr, w, h, wdwidth: mem_wdwidth(w), stand: 0, nplanes: 32 }
    }

    fn is_screen(&self) -> bool {
        self.addr == 0
    }

    /// Row stride in **pixels** of this device-format memory MFDB (one
    /// `u32` per pixel), derived from `fd_wdwidth` (16-pixel words, so the
    /// row is padded up to cover `fd_w`). The copy uses the MFDB's own
    /// geometry — never the screen stride.
    fn mem_stride_px(&self) -> usize {
        (self.wdwidth.max(0) as usize) * 16
    }

    /// Number of `u32` pixel words the buffer holds (`stride × height`);
    /// every memory access is bounds-checked against this.
    fn mem_capacity(&self) -> usize {
        self.mem_stride_px() * (self.h.max(0) as usize)
    }

    /// Read the 32-bpp pixel at `(col, row)` of a memory MFDB, or `0` when
    /// outside the `fd_w × fd_h` extent (or the buffer).
    ///
    /// # Safety
    /// `addr` must point at a `mem_capacity()`-word `u32` buffer.
    #[inline]
    unsafe fn get_mem(&self, col: i32, row: i32) -> u32 {
        if col < 0 || row < 0 || col >= self.w as i32 || row >= self.h as i32 {
            return 0;
        }
        let idx = row as usize * self.mem_stride_px() + col as usize;
        if idx >= self.mem_capacity() {
            return 0;
        }
        // SAFETY: `idx < mem_capacity()`, within the caller-owned buffer.
        unsafe { (self.addr as *const u32).add(idx).read() }
    }

    /// Write the 32-bpp pixel at `(col, row)` of a memory MFDB; a
    /// coordinate outside the `fd_w × fd_h` extent (or the buffer) is
    /// ignored.
    ///
    /// # Safety
    /// As [`get_mem`](Self::get_mem).
    #[inline]
    unsafe fn put_mem(&self, col: i32, row: i32, val: u32) {
        if col < 0 || row < 0 || col >= self.w as i32 || row >= self.h as i32 {
            return;
        }
        let idx = row as usize * self.mem_stride_px() + col as usize;
        if idx >= self.mem_capacity() {
            return;
        }
        // SAFETY: `idx < mem_capacity()`, within the caller-owned buffer.
        unsafe { (self.addr as *mut u32).add(idx).write(val) }
    }
}

/// Words-per-row (16-pixel units) of a device-format memory MFDB of width
/// `w` — the `fd_wdwidth = (fd_w + 15) / 16` ABI formula.
#[inline]
fn mem_wdwidth(w: i16) -> i16 {
    ((w.max(0) as i32 + 15) / 16) as i16
}

/// The 16-entry BitBlt logic-op table, `D' = op(S, D)`, on the raw pixel
/// word (`docs/api/vdi/raster/_raster.md`, "Logic operations"). The same
/// table the screen backend applies for Screen→Screen; kept here so the
/// memory (off-screen) paths need no `kernel` dependency and so all four
/// src/dst combinations share one definition. Only the low nibble selects
/// the op (`PAT_FLAG` is already stripped by the caller).
#[inline]
fn raster_op(op: u8, s: u32, d: u32) -> u32 {
    match op & 0x0f {
        0 => 0,          // BM_ALL_WHITE   D' = 0
        1 => s & d,      // BM_S_AND_D
        2 => s & !d,     // BM_S_AND_NOTD
        3 => s,          // BM_S_ONLY      (replace)
        4 => !s & d,     // BM_NOTS_AND_D
        5 => d,          // BM_D_ONLY      (no-op)
        6 => s ^ d,      // BM_S_XOR_D     (XOR)
        7 => s | d,      // BM_S_OR_D      (OR)
        8 => !(s | d),   // BM_NOT_SORD
        9 => !(s ^ d),   // BM_NOT_SXORD
        10 => !d,        // BM_NOT_D
        11 => s | !d,    // BM_S_OR_NOTD
        12 => !s,        // BM_NOT_S
        13 => !s | d,    // BM_NOTS_OR_D
        14 => !(s & d),  // BM_NOT_SANDD
        _ => !0,         // BM_ALL_BLACK   D' = ~0
    }
}

/// Normalise a rectangle's two corners read from `PTSIN[base..base+3]`.
fn rect_from_ptsin(pb: &Vdipb, base: usize) -> Rect {
    let mut x1 = pb.ptsin(base);
    let mut y1 = pb.ptsin(base + 1);
    let mut x2 = pb.ptsin(base + 2);
    let mut y2 = pb.ptsin(base + 3);
    if x1 > x2 {
        core::mem::swap(&mut x1, &mut x2);
    }
    if y1 > y2 {
        core::mem::swap(&mut y1, &mut y2);
    }
    Rect { x: x1, y: y1, w: x2 - x1 + 1, h: y2 - y1 + 1 }
}

/// Opcode 121 — `vrt_cpyfm`: copy a 1-plane monochrome source into the
/// colour screen, expanding to `INTIN[1]` (fg) / `INTIN[2]` (bg).
pub fn vrt_cpyfm(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend, ncol: u16) {
    // SAFETY: native MFDB-pointer convention (see module docs).
    let (src, dst) = unsafe {
        (Mfdb::from_ptr(pb.contrl_u64(7)), Mfdb::from_ptr(pb.contrl_u64(11)))
    };
    // Bring-up: 1-plane source into the screen destination.
    if src.is_screen() || src.nplanes != 1 || !dst.is_screen() {
        return;
    }

    // `INTIN[0]` is the `MD_*` write mode (EmuTOS `vdi_defs.h:77-80`,
    // `cpy_raster:1397-1430`): MD_REPLACE=1, MD_TRANS=2, MD_XOR=3,
    // MD_ERASE=4. Any other value (0 or ≥5) is a silent no-op — EmuTOS'
    // `default: return` — so we must not blit. (The old code decoded 1/2
    // swapped and folded XOR/ERASE onto transparent; corrected here per
    // `_raster.md:195,208-213,286-288`.)
    let mode = match pb.intin(0) {
        1 => WriteMode::Replace,        // fg on 1-bits, bg on 0-bits
        2 => WriteMode::Transparent,    // fg on 1-bits, leave 0-bits
        3 => WriteMode::Xor,            // complement on 1-bits, leave 0-bits
        4 => WriteMode::RevTransparent, // leave 1-bits, bg on 0-bits (MD_ERASE)
        _ => return,                    // invalid → no-op
    };
    let fg = validate_color_index(pb.intin(1), ncol);
    let bg = validate_color_index(pb.intin(2), ncol);

    let s = rect_from_ptsin(pb, 0); // source rect
    let d = rect_from_ptsin(pb, 4); // dest rect (same extent)

    // SAFETY: the source MFDB describes `h * wdwidth` words at `addr`.
    let words = (src.h as usize) * (src.wdwidth as usize);
    let bits = unsafe { core::slice::from_raw_parts(src.addr as *const u16, words) };

    backend.blit_mono(
        Rect { x: d.x, y: d.y, w: s.w, h: s.h },
        MonoSrc { bits, stride_words: src.wdwidth as u16, x: s.x, y: s.y },
        fg,
        bg,
        mode,
        vwk.clip_rect(),
    );
}

/// Opcode 109 — `vro_cpyfm`: opaque copy. Handles all four source/
/// destination combinations of screen and memory MFDB with the BitBlt
/// logic op in `INTIN[0]`: Screen↔Screen, Screen→Memory (`bb_save`),
/// Memory→Screen (`bb_restore`), Memory↔Memory. The destination extent
/// is the source extent; clipping applies **only** when the destination
/// is the screen (`_raster.md` edge case 8).
pub fn vro_cpyfm(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    // SAFETY: native MFDB-pointer convention (see module docs).
    let (src, dst) = unsafe {
        (Mfdb::from_ptr(pb.contrl_u64(7)), Mfdb::from_ptr(pb.contrl_u64(11)))
    };
    // `INTIN[0]` carries the raster op in the low 4 bits, `PAT_FLAG`
    // (bit 4, 0x10) in bit 4, and is invalid if anything above bit 4 is
    // set (`cpy_raster:1341-1349`). Strip `PAT_FLAG` before dispatch — per
    // D-082 it never enables a pattern here — and pass the 16-entry op.
    let mut mode = pb.intin(0);
    if mode & !0x1F != 0 {
        return; // bits above PAT_FLAG set → invalid, no-op
    }
    mode &= !0x10; // strip PAT_FLAG (bit 4); D-082: never blits a pattern
    let op = (mode & 0x0F) as u8;
    let s = rect_from_ptsin(pb, 0); // source rect (extent)
    let d = rect_from_ptsin(pb, 4); // destination upper-left
    match (src.is_screen(), dst.is_screen()) {
        // Screen→Screen: the backend block-copy (overlap-safe, cache-clean).
        (true, true) => backend.copy_rect(s, d.x, d.y, op, vwk.clip_rect()),
        // Screen→Memory (bb_save): read the screen, write the buffer.
        (true, false) => screen_to_mem(backend, &dst, s, d.x, d.y, op),
        // Memory→Screen (bb_restore): read the buffer, write the screen.
        (false, true) => mem_to_screen(backend, &src, s, d.x, d.y, op, vwk.clip_rect()),
        // Memory→Memory: pure buffer copy (overlap-safe when co-located).
        (false, false) => mem_to_mem(&src, &dst, s, d.x, d.y, op),
    }
}

/// Screen→Memory copy (`vro_cpyfm` `bb_save` direction). The destination
/// is a device-format memory MFDB; **no clipping** (dest is not the
/// screen). Source rect `(s.x, s.y, s.w, s.h)` on screen → memory origin
/// `(dx, dy)`, applying `op` against the buffer's current pixel.
fn screen_to_mem(backend: &dyn RasterBackend, dst: &Mfdb, s: Rect, dx: i16, dy: i16, op: u8) {
    for r in 0..s.h as i32 {
        for c in 0..s.w as i32 {
            let sp = backend.get_device_pixel((s.x as i32 + c) as i16, (s.y as i32 + r) as i16);
            // SAFETY: `dst` is a valid device-format memory MFDB.
            let dp = unsafe { dst.get_mem(dx as i32 + c, dy as i32 + r) };
            // SAFETY: as above; `put_mem` bounds-checks the index.
            unsafe { dst.put_mem(dx as i32 + c, dy as i32 + r, raster_op(op, sp, dp)) };
        }
    }
}

/// Memory→Screen copy (`vro_cpyfm` `bb_restore` direction). The
/// destination is the screen, so the write is clipped to `clip` ∩ the
/// screen (`_raster.md` edge case 8). Source memory rect at `(s.x, s.y)`
/// → screen origin `(dx, dy)`, applying `op` against the live screen
/// pixel, then flushing the touched region to the point of coherency.
fn mem_to_screen(
    backend: &dyn RasterBackend,
    src: &Mfdb,
    s: Rect,
    dx: i16,
    dy: i16,
    op: u8,
    clip: ClipRect,
) {
    let (sw, sh) = (backend.width() as i32, backend.height() as i32);
    // Destination box (dx, dy, s.w, s.h) ∩ clip ∩ screen (inclusive).
    let x0 = (dx as i32).max(clip.xmn as i32).max(0);
    let y0 = (dy as i32).max(clip.ymn as i32).max(0);
    let x1 = (dx as i32 + s.w as i32 - 1).min(clip.xmx as i32).min(sw - 1);
    let y1 = (dy as i32 + s.h as i32 - 1).min(clip.ymx as i32).min(sh - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }
    for ty in y0..=y1 {
        for tx in x0..=x1 {
            let sc = s.x as i32 + (tx - dx as i32);
            let sr = s.y as i32 + (ty - dy as i32);
            // SAFETY: `src` is a valid device-format memory MFDB.
            let sp = unsafe { src.get_mem(sc, sr) };
            let dp = backend.get_device_pixel(tx as i16, ty as i16);
            backend.put_device_pixel(tx as i16, ty as i16, raster_op(op, sp, dp));
        }
    }
    backend.flush_rect(x0 as i16, y0 as i16, x1 as i16, y1 as i16);
}

/// Memory→Memory copy. Overlap-safe by copy-direction choice when the two
/// MFDBs share a buffer (`_raster.md` edge case 5); no clipping.
fn mem_to_mem(src: &Mfdb, dst: &Mfdb, s: Rect, dx: i16, dy: i16, op: u8) {
    let overlap = src.addr == dst.addr;
    let down = overlap && (dy as i32) > s.y as i32;
    let right = overlap && (dx as i32) > s.x as i32;
    for ri in 0..s.h as i32 {
        let r = if down { s.h as i32 - 1 - ri } else { ri };
        for ci in 0..s.w as i32 {
            let c = if right { s.w as i32 - 1 - ci } else { ci };
            // SAFETY: both MFDBs are valid device-format memory buffers.
            let sp = unsafe { src.get_mem(s.x as i32 + c, s.y as i32 + r) };
            let dp = unsafe { dst.get_mem(dx as i32 + c, dy as i32 + r) };
            // SAFETY: as above; `put_mem` bounds-checks the index.
            unsafe { dst.put_mem(dx as i32 + c, dy as i32 + r, raster_op(op, sp, dp)) };
        }
    }
}

/// Number of `u32` pixel words a `bb_save` buffer for a `w × h` screen
/// region needs (device-format, one word per pixel, rows padded to 16 px).
/// AES allocates exactly this from the kernel heap.
pub fn bb_save_words(w: i16, h: i16) -> usize {
    (mem_wdwidth(w) as usize) * 16 * (h.max(0) as usize)
}

/// AES `bb_save` (EmuTOS `gemgsxif.c` `bb_set`, save direction): copy the
/// screen rectangle `(x, y, w, h)` into the caller-owned 32-bpp
/// device-format buffer `mem` (a Screen→Memory `vro_cpyfm`, `S_ONLY` /
/// replace). Called directly by the AES form manager rather than through a
/// re-entrant trap.
///
/// # Safety
/// `mem` must point at a buffer of at least [`bb_save_words`]`(w, h)`
/// `u32` words that stays alive until the matching [`bb_restore`].
pub unsafe fn bb_save(backend: &dyn RasterBackend, mem: *mut u32, x: i16, y: i16, w: i16, h: i16) {
    let dst = Mfdb::device_mem(mem as u64, w, h);
    screen_to_mem(backend, &dst, Rect { x, y, w, h }, 0, 0, OP_REPLACE);
}

/// AES `bb_restore`: copy the device-format buffer `mem` (filled by
/// [`bb_save`]) back to the screen rectangle `(x, y, w, h)` — a
/// Memory→Screen `vro_cpyfm` replace, clipped to `clip`.
///
/// # Safety
/// As [`bb_save`]; `mem` must hold [`bb_save_words`]`(w, h)` words.
pub unsafe fn bb_restore(
    backend: &dyn RasterBackend,
    mem: *const u32,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    clip: ClipRect,
) {
    let src = Mfdb::device_mem(mem as u64, w, h);
    mem_to_screen(backend, &src, Rect { x: 0, y: 0, w, h }, x, y, OP_REPLACE, clip);
}

/// Opcode 110 — `vr_trnfm`: transform a bitmap between **standard**
/// (plane-sequential, machine-independent) and **device-dependent**
/// (word-interleaved) form. The destination's `fd_stand` is forced to
/// the opposite of the source's; works in place when the addresses
/// coincide. Faithful port of EmuTOS `vdi_vr_trnfm` (`vdi_raster.c`).
pub fn vr_trnfm(pb: &Vdipb) {
    // SAFETY: native MFDB-pointer convention (see module docs).
    let src = unsafe { Mfdb::from_ptr(pb.contrl_u64(7)) };
    let dst_ptr = pb.contrl_u64(11);
    if src.addr == 0 || dst_ptr == 0 {
        return;
    }
    // SAFETY: dst MFDB layout matches Mfdb (addr@0, …, stand@14).
    let dst = unsafe { Mfdb::from_ptr(dst_ptr) };
    if dst.addr == 0 {
        return;
    }

    let planes = src.nplanes.max(0) as usize;
    let size = (src.h as usize) * (src.wdwidth as usize); // words per plane
    if planes == 0 || size == 0 {
        return;
    }
    let (outer, inner) = if src.stand != 0 {
        (planes, size) // standard → device
    } else {
        (size, planes) // device → standard
    };

    let s = src.addr as *const u16;
    let d = dst.addr as *mut u16;
    let inplace = src.addr == dst.addr;

    // Force the destination form opposite to the source (offset 14 = fd_stand).
    let new_stand: i16 = if src.stand != 0 { 0 } else { 1 };
    // SAFETY: dst MFDB is valid; fd_stand is the WORD at byte offset 14.
    unsafe { ((dst_ptr as *mut u8).add(14) as *mut i16).write_unaligned(new_stand) };

    if !inplace {
        // Simple transpose: read the source linearly, scatter into the
        // destination with stride `outer`.
        // SAFETY: both buffers hold `size*planes` words (MFDB geometry).
        unsafe {
            let mut si = 0usize;
            for i in 0..outer {
                let mut wi = i;
                for _ in 0..inner {
                    *d.add(wi) = *s.add(si);
                    si += 1;
                    wi += outer;
                }
            }
        }
        return;
    }

    // In-place: mono has no standard/device difference.
    if planes == 1 {
        return;
    }
    if outer <= 1 {
        return;
    }
    // Faithful port of the EmuTOS in-place rotation (no scratch buffer).
    // SAFETY: the buffer holds `size*planes` words; all index motion stays
    // within it, mirroring the reference algorithm 1:1.
    unsafe {
        let outer = (outer - 1) as isize;
        let mut inner = inner as isize - 1;
        let mut src = d; // src == dst buffer
        let mut work = src;
        while inner >= 0 {
            let mut count: isize = 0;
            for _ in 0..outer {
                src = src.offset(inner + 1);
                let temp = *src;
                let mut dst = src;
                work = src;
                count += inner;
                for _ in 0..count {
                    work = dst;
                    dst = dst.offset(-1);
                    *work = *dst;
                }
                *dst = temp;
            }
            src = work;
            inner -= 1;
        }
    }
}
