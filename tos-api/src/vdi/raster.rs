//! VDI raster operations — `vrt_cpyfm` (transparent mono→colour blit)
//! and `vro_cpyfm` (opaque copy). Spec: `docs/api/vdi/raster/_raster.md`.
//!
//! **MFDB pointer convention (caller-ABI aware).** gemlib packs VDI
//! pointer `n` at `CONTRL[7 + n * N_PTRINTS]`, with `N_PTRINTS =
//! sizeof(void*) / sizeof(short)` (`gem_vdiP.h:177,204`). For an **LP64**
//! caller `N_PTRINTS = 4`, so the source MFDB pointer occupies
//! `CONTRL[7..10]` and the destination `CONTRL[11..14]` (8 bytes each).
//! For an **ILP32** caller `N_PTRINTS = 2`: source at `CONTRL[7..8]`,
//! destination at `CONTRL[9..10]` (4 bytes each). Both the pointer read
//! and the MFDB struct-field offsets shift with the ABI — see
//! [`Vdipb::contrl_ptr`] and [`Mfdb::from_ptr`]. `fd_addr == NULL` means
//! the screen device under either width.
//!
//! Scope: `vrt_cpyfm` with a 1-plane source (icons/sprites), all four
//! `MD_*` write modes (replace / transparent / XOR / erase); `vro_cpyfm`
//! for all four source/destination combinations with the full 16-entry
//! logic-op table — Screen↔Screen, **Screen→Memory** (AES `bb_save`),
//! **Memory→Screen** (`bb_restore`), and **Memory↔Memory**.
//!
//! **NewTOS memory-MFDB format (device form, `fd_stand = 0`).** A memory
//! MFDB (`fd_addr != 0`) is a **16-bit RGB565-chunky truecolor** buffer:
//! one `UWORD` (`RRRRRGGGGG0BBBBB`, the green LSB / bit 5 always 0 — the
//! Falcon truecolor pixel, `vdi_defs.h:156`, `vdi_col.c:599`) per pixel.
//! This is the app-visible **wireformat** EmuTOS' `vro_cpyfm16` /
//! `vrt_cpyfm16` operate on raw (`vdi_raster.c:975-1221,1226-1326`): a
//! Falcon program that pokes RGB565 pixels into its own MFDB and blits
//! expects exactly this layout — it is observable, not an implementation
//! detail (`docs/COMPATIBILITY.md`). The row stride is derived from
//! `fd_wdwidth` (16-pixel words, so the row is rounded up to a multiple of
//! 16 px, mirroring EmuTOS `bb_set`'s word alignment) — a memory copy uses
//! the MFDB's *own* geometry, never the screen stride. `fd_nplanes = 16`
//! marks the depth ([`CHUNKY_PLANES`]); with it the classic interleaved
//! identity `bytes/line = fd_wdwidth * fd_nplanes / 8 * 16 = fd_wdwidth *
//! 32` holds.
//!
//! The **physical framebuffer stays 32-bpp XRGB8888** (an internal detail
//! per `docs/COMPATIBILITY.md`); the 16↔32 conversion happens only at the
//! screen boundary inside [`screen_to_mem`] / [`mem_to_screen`], via the
//! backend's [`RasterBackend::device_to_rgb565`] /
//! [`RasterBackend::rgb565_to_device`]. Because the screen is
//! 5-bit-quantised (every VDI draw resolves through the RGB555-in-565
//! pseudo-palette), that round-trip is **lossless**, so `bb_save`/
//! `bb_restore` reproduce the screen bit-for-bit.
//!
//! The buffer is caller-owned (AES `bb_save` allocates it); the VDI only
//! reads/writes through `fd_addr`, always bounds-checked against
//! `fd_w × fd_h`.

use alloc::alloc::{alloc_zeroed, dealloc, Layout};

use super::backend::{ClipRect, MonoBits, MonoSrc, RasterBackend, Rect, WriteMode};
use super::vwk::{validate_color_index, Vwk};
use super::Vdipb;
use crate::bdos::process::Abi;

/// The `S_ONLY` logic op (`INTIN[0] = 3`, `D' = S`, replace) — the op AES
/// `bb_save`/`bb_restore` use (EmuTOS `gemgsxif.c` `bb_set`).
const OP_REPLACE: u8 = 3;

/// The plane count of a NewTOS device-format memory MFDB (`fd_nplanes`) —
/// **16**, the Falcon truecolor depth (`v_planes`, `bios/videl.c:320-341`).
/// The screen resolves to the same count via
/// [`RasterBackend::planes`](super::backend::RasterBackend::planes). Used
/// both to stamp `device_mem` MFDBs and as the `setup_info`/`cpy_raster`
/// plane-equality gate for `vro_cpyfm` (`vdi_raster.c:963,1360`, AUD-V5-004).
const CHUNKY_PLANES: i16 = 16;

/// The largest plane count EmuTOS still handles on its **palette** BitBlt
/// path. `cpy_raster` dispatches to the truecolor `vro_cpyfm16` /
/// `vrt_cpyfm16` routines for `plane_ct > 8` (`vdi_raster.c:1363-1368,
/// 1432-1437`), and those two re-read `INTIN[0]` unstripped — which is why
/// the threshold is observable and not just an internal detail.
const PALETTE_PLANES_MAX: i16 = 8;

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
    /// Read an MFDB from its pointer at the caller's `abi` width (0 ⇒ a
    /// null/screen MFDB). The struct is `void *fd_addr; short fd_w, fd_h,
    /// fd_wdwidth, fd_stand, fd_nplanes, …` (`mt_gem.h:2757-2768`), so the
    /// pointer width **and** the following WORD-field offsets shift with
    /// the ABI: LP64 → `fd_addr` 8B @0, fields @8/10/12/14/16; ILP32 →
    /// `fd_addr` 4B @0, fields @4/6/8/10/12. A NULL `fd_addr` (screen
    /// MFDB) reads as `addr == 0` under either width.
    ///
    /// # Safety
    /// `ptr` must be 0 or a valid MFDB of `abi`'s layout.
    unsafe fn from_ptr(ptr: u64, abi: Abi) -> Self {
        if ptr == 0 {
            return Mfdb { addr: 0, w: 0, h: 0, wdwidth: 0, stand: 0, nplanes: 0 };
        }
        // SAFETY: caller guarantees a valid MFDB of `abi`'s layout.
        unsafe {
            let p = ptr as *const u8;
            let word = |off: usize| (p.add(off) as *const i16).read_unaligned();
            match abi {
                Abi::Lp64 => Mfdb {
                    addr: (p as *const u64).read_unaligned(),
                    w: word(8),
                    h: word(10),
                    wdwidth: word(12),
                    stand: word(14),
                    nplanes: word(16),
                },
                Abi::Ilp32 => Mfdb {
                    addr: (p as *const u32).read_unaligned() as u64,
                    w: word(4),
                    h: word(6),
                    wdwidth: word(8),
                    stand: word(10),
                    nplanes: word(12),
                },
            }
        }
    }

    /// Build a NewTOS 16-bit RGB565 device-format memory MFDB over a
    /// caller-owned pixel buffer of `w × h` `u16` pixels (AES `bb_save`):
    /// `stand = 0` (device), `nplanes = 16` ([`CHUNKY_PLANES`]),
    /// `wdwidth = (w + 15) / 16`.
    fn device_mem(addr: u64, w: i16, h: i16) -> Self {
        Mfdb { addr, w, h, wdwidth: mem_wdwidth(w), stand: 0, nplanes: CHUNKY_PLANES }
    }

    fn is_screen(&self) -> bool {
        self.addr == 0
    }

    /// Row stride in **pixels** of this device-format memory MFDB (one
    /// `u16` per pixel), derived from `fd_wdwidth` (16-pixel words, so the
    /// row is padded up to cover `fd_w`). The copy uses the MFDB's own
    /// geometry — never the screen stride.
    fn mem_stride_px(&self) -> usize {
        (self.wdwidth.max(0) as usize) * 16
    }

    /// Number of `u16` pixel words the buffer holds (`stride × height`);
    /// every memory access is bounds-checked against this.
    fn mem_capacity(&self) -> usize {
        self.mem_stride_px() * (self.h.max(0) as usize)
    }

    /// Read the 16-bit RGB565 pixel at `(col, row)` of a memory MFDB, or `0`
    /// when outside the `fd_w × fd_h` extent (or the buffer).
    ///
    /// # Safety
    /// `addr` must point at a `mem_capacity()`-word `u16` buffer.
    #[inline]
    unsafe fn get_mem(&self, col: i32, row: i32) -> u16 {
        if col < 0 || row < 0 || col >= self.w as i32 || row >= self.h as i32 {
            return 0;
        }
        let idx = row as usize * self.mem_stride_px() + col as usize;
        if idx >= self.mem_capacity() {
            return 0;
        }
        // SAFETY: `idx < mem_capacity()`, within the caller-owned buffer.
        unsafe { (self.addr as *const u16).add(idx).read_unaligned() }
    }

    /// Write the 16-bit RGB565 pixel at `(col, row)` of a memory MFDB; a
    /// coordinate outside the `fd_w × fd_h` extent (or the buffer) is
    /// ignored.
    ///
    /// # Safety
    /// As [`get_mem`](Self::get_mem).
    #[inline]
    unsafe fn put_mem(&self, col: i32, row: i32, val: u16) {
        if col < 0 || row < 0 || col >= self.w as i32 || row >= self.h as i32 {
            return;
        }
        let idx = row as usize * self.mem_stride_px() + col as usize;
        if idx >= self.mem_capacity() {
            return;
        }
        // SAFETY: `idx < mem_capacity()`, within the caller-owned buffer.
        unsafe { (self.addr as *mut u16).add(idx).write_unaligned(val) }
    }
}

/// Words-per-row (16-pixel units) of a device-format memory MFDB of width
/// `w` — the `fd_wdwidth = (fd_w + 15) / 16` ABI formula.
#[inline]
fn mem_wdwidth(w: i16) -> i16 {
    ((w.max(0) as i32 + 15) / 16) as i16
}

/// The 16-entry BitBlt logic-op table, `D' = op(S, D)`, acting **literally
/// on the 16-bit RGB565 pixel word** — EmuTOS `vro_cpyfm16` operates the
/// exact same table on `UWORD` pixels (`vdi_raster.c:1008-1220`). The
/// memory side of every copy is now 16-bit RGB565 (see module docs), and
/// the screen is converted to/from RGB565 at the boundary, so a
/// memory-MFDB copy always computes the op in the 16-bit domain, matching
/// the Falcon truecolor VDI. Kept here so the off-screen paths need no
/// `kernel` dependency. Only the low nibble selects the op (`PAT_FLAG` is
/// already stripped by the caller).
#[inline]
fn raster_op(op: u8, s: u16, d: u16) -> u16 {
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

/// Effective plane count of an MFDB for the `s_nxwd == d_nxwd` /
/// `plane_ct <= 16` gates EmuTOS applies in `setup_info` / `cpy_raster`
/// (`vdi_raster.c:922,937,963,1360`). The screen resolves to `v_planes`
/// (the backend's [`planes`](RasterBackend::planes), 16 under Falcon
/// truecolor); a memory MFDB carries its own `fd_nplanes`.
#[inline]
fn effective_planes(mfdb: &Mfdb, backend: &dyn RasterBackend) -> i16 {
    if mfdb.is_screen() {
        backend.planes() as i16
    } else {
        mfdb.nplanes
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

/// Opcode 121 — `vrt_cpyfm`: copy a 1-plane monochrome source into a colour
/// destination, expanding to `INTIN[1]` (fg) / `INTIN[2]` (bg). The
/// destination may be the **screen** (the common case: icons/sprites onto
/// the display) or a **memory MFDB** (16-bit RGB565, AUD-V5-005).
pub fn vrt_cpyfm(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend, ncol: u16) {
    let abi = pb.abi();
    // SAFETY: caller-ABI MFDB-pointer convention (see module docs); the
    // src pointer is at CONTRL[7], the dst at 7 + N_PTRINTS.
    let (src, dst) = unsafe {
        (Mfdb::from_ptr(pb.contrl_ptr(0), abi), Mfdb::from_ptr(pb.contrl_ptr(1), abi))
    };
    // Source must be a 1-plane memory raster (`cpy_raster:1384`,
    // `s_nxwd != 2` ⇒ reject); a screen source is not a mono form.
    if src.is_screen() || src.nplanes != 1 {
        return;
    }
    // `setup_info`: the destination plane count must be ≤ 16
    // (`vdi_raster.c:963`), else there is nothing to do.
    if effective_planes(&dst, backend) > CHUNKY_PLANES {
        return;
    }

    // `INTIN[0]` is the `MD_*` write mode (EmuTOS `vdi_defs.h:77-80`,
    // `cpy_raster:1397-1430`): MD_REPLACE=1, MD_TRANS=2, MD_XOR=3,
    // MD_ERASE=4. Strip `PAT_FLAG` (bit 4) and reject anything above the
    // low 5 bits before the switch, exactly like `vro_cpyfm` and EmuTOS
    // `cpy_raster:1341,1347` (AUD-V5-001): a legal `MD_x|PAT_FLAG` must
    // still copy, and a bit-5+ value is invalid. Any other value (0 or ≥5)
    // is a silent no-op — EmuTOS' `default: return`.
    let raw = pb.intin(0);
    if raw & !0x1F != 0 {
        return; // bits above PAT_FLAG set → invalid, no-op
    }
    let mode = match raw & !0x10 {
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

    // SAFETY: the source MFDB describes `h * wdwidth` words at `addr`. The
    // MFDB is the caller's, so `fd_addr` may sit at an odd address; `MonoBits`
    // reads the words without requiring alignment.
    let words = (src.h as usize) * (src.wdwidth as usize);
    let bits = unsafe { MonoBits::from_raw(src.addr as *const u8, words) };
    let mono = MonoSrc { bits, stride_words: src.wdwidth as u16, x: s.x, y: s.y };

    if dst.is_screen() {
        // Screen destination: the backend expands the mono source into the
        // display, clipped to the workstation clip (the only clipped case,
        // `setup_info` `use_clip` when dst is screen).
        backend.blit_mono(Rect { x: d.x, y: d.y, w: s.w, h: s.h }, mono, fg, bg, mode, vwk.clip_rect());
    } else {
        // Memory destination (AUD-V5-005): expand into the 16-bit RGB565
        // buffer with the same `MD_*` semantics, resolving fg/bg through the
        // active pseudo-palette (`vrt_cpyfm16`, `fgcol = palette[fg_col]`,
        // `vdi_raster.c:1234-1236`). No clipping (dst is not the screen).
        //
        // AUD-V5-008 (`MAP_COL` fold): EmuTOS resolves `palette[MAP_COL[fg]]`
        // — `cpy_raster` maps `fg_col = MAP_COL[validate_color_index(INTIN[1])]`
        // (`vdi_raster.c:1390-1395`) before `vrt_cpyfm16` indexes the
        // pseudo-palette. NewTOS seeds the active pseudo-palette **in VDI-pen
        // order** (`vdi_backend.rs` `videl_palette1`/`videl_palette2` at index
        // = pen), i.e. the `MAP_COL` permutation is folded into the palette
        // seed, so `palette[fg]` here equals EmuTOS' `palette[MAP_COL[fg]]`
        // pixel-for-pixel — and identically to how every other primitive
        // (line/fill/text/`blit_mono`) resolves the same pen. The fold holds
        // for **all** pens (seed is a full 0..255 identity-ordered table), so
        // V5-008 stays a folded-acceptable, unobservable deviation; adding a
        // non-identity `MAP_COL` here would desync from the seeded palette and
        // break the desktop's colours.
        let fg16 = backend.pel_to_device16(fg);
        let bg16 = backend.pel_to_device16(bg);
        mono_to_mem(&dst, mono, d.x, d.y, s.w, s.h, mode, fg16, bg16);
    }
}

/// `vrt_cpyfm` memory-destination expansion (EmuTOS `vrt_cpyfm16`,
/// `vdi_raster.c:1226-1326`): blit the 1-plane `mono` source into the
/// 16-bit RGB565 memory MFDB `dst` at origin `(dx, dy)` over `w × h`
/// pixels, per the `MD_*` write mode. `fg16`/`bg16` are the already-
/// resolved RGB565 pixel words. No clipping.
#[allow(clippy::too_many_arguments)]
fn mono_to_mem(dst: &Mfdb, mono: MonoSrc, dx: i16, dy: i16, w: i16, h: i16, mode: WriteMode, fg16: u16, bg16: u16) {
    let stride = mono.stride_words as usize;
    for r in 0..h as i32 {
        let sy = mono.y as i32 + r;
        for c in 0..w as i32 {
            let sx = mono.x as i32 + c;
            let widx = sy as usize * stride + (sx as usize) / 16;
            let set = mono.bits.word(widx).is_some_and(|word| word & (0x8000u16 >> (sx & 15)) != 0);
            let (col, row) = (dx as i32 + c, dy as i32 + r);
            match mode {
                // MD_REPLACE: fg on 1-bits, bg on 0-bits (opaque).
                WriteMode::Replace => {
                    // SAFETY: `dst` is a valid device-format memory MFDB.
                    unsafe { dst.put_mem(col, row, if set { fg16 } else { bg16 }) };
                }
                // MD_TRANS: fg on 1-bits; 0-bits untouched.
                WriteMode::Transparent => {
                    if set {
                        // SAFETY: as above.
                        unsafe { dst.put_mem(col, row, fg16) };
                    }
                }
                // MD_XOR: complement the destination on 1-bits.
                WriteMode::Xor => {
                    if set {
                        // SAFETY: as above; `get_mem` bounds-checks.
                        unsafe { dst.put_mem(col, row, !dst.get_mem(col, row)) };
                    }
                }
                // MD_ERASE (reverse-transparent): bg on 0-bits; 1-bits alone.
                WriteMode::RevTransparent => {
                    if !set {
                        // SAFETY: as above.
                        unsafe { dst.put_mem(col, row, bg16) };
                    }
                }
            }
        }
    }
}

/// Opcode 109 — `vro_cpyfm`: opaque copy. Handles all four source/
/// destination combinations of screen and memory MFDB with the BitBlt
/// logic op in `INTIN[0]`: Screen↔Screen, Screen→Memory (`bb_save`),
/// Memory→Screen (`bb_restore`), Memory↔Memory. The destination extent
/// is the source extent; clipping applies **only** when the destination
/// is the screen (`_raster.md` edge case 8).
pub fn vro_cpyfm(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let abi = pb.abi();
    // SAFETY: caller-ABI MFDB-pointer convention (see module docs); the
    // src pointer is at CONTRL[7], the dst at 7 + N_PTRINTS.
    let (src, dst) = unsafe {
        (Mfdb::from_ptr(pb.contrl_ptr(0), abi), Mfdb::from_ptr(pb.contrl_ptr(1), abi))
    };
    // `INTIN[0]` carries the raster op in the low 4 bits and `PAT_FLAG`
    // (bit 4, 0x10); anything above bit 4 makes the mode invalid
    // (`cpy_raster:1341-1342`). The `PAT_FLAG` decision itself depends on
    // the plane count and is therefore taken below, after the plane gates.
    let raw_mode = pb.intin(0);
    if raw_mode & !0x1F != 0 {
        return; // bits above PAT_FLAG set → invalid, no-op
    }

    // Opaque-copy plane gate (AUD-V5-004), EmuTOS `setup_info` +
    // `cpy_raster:1360`: the destination plane count must be ≤ 16
    // (`vdi_raster.c:963`) and the source/destination plane counts must
    // match (`s_nxwd != d_nxwd` ⇒ return). The screen resolves to
    // `v_planes` (16), a memory MFDB to its own `fd_nplanes`; a
    // `device_mem` buffer is stamped [`CHUNKY_PLANES`] (16), so the
    // Screen↔Memory `bb_save`/`bb_restore` case (16 vs 16) still passes.
    let (sp, dp) = (effective_planes(&src, backend), effective_planes(&dst, backend));
    if dp > CHUNKY_PLANES || sp != dp {
        return;
    }

    // `PAT_FLAG` (bit 4) on the **truecolor** path is not merely stripped —
    // it kills the copy. `cpy_raster` strips it into its own local `mode`
    // (`vdi_raster.c:1347`) but then hands the truecolor branch nothing:
    // `vro_cpyfm16` re-reads the *unstripped* `INTIN[0]`
    // (`vdi_raster.c:980`), so a mode with bit 4 set (0x10..0x1F) matches
    // no arm of its 16-entry switch — which has no `default:`
    // (`vdi_raster.c:1008-1219`) — and the destination is left untouched.
    // Only the palette path (`plane_ct <= 8`, `bit_blt`) sees the stripped
    // mode, which is D-082's strip-only contract. Measured on a real Atari
    // TT running EmuTOS 1.4 with `CONF_WITH_VDI_16BIT`: `tosdump` case 144
    // (`INTIN[0] = 0x13`, 16-plane forms) leaves the destination buffer
    // bit-identical to its fill, while case 127 (`INTIN[0] = 3`, same
    // geometry) copies 64 words.
    if dp > PALETTE_PLANES_MAX && raw_mode & 0x10 != 0 {
        return; // PAT_FLAG on the truecolor path → nothing is copied
    }
    let op = (raw_mode & 0x0F) as u8;

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
            // Convert the raw 32-bpp screen pixel to the 16-bit RGB565
            // memory wireformat, then apply the logic op in the 16-bit
            // domain (EmuTOS `vro_cpyfm16`, all ops on `UWORD` pixels).
            let sp = backend
                .device_to_rgb565(backend.get_device_pixel((s.x as i32 + c) as i16, (s.y as i32 + r) as i16));
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
            let sp = unsafe { src.get_mem(sc, sr) }; // 16-bit RGB565
            // Bring the live screen pixel into the 16-bit domain, apply the
            // op there, then expand the result back to a device pixel — the
            // op runs on `UWORD` pixels exactly as EmuTOS `vro_cpyfm16`.
            let dp = backend.device_to_rgb565(backend.get_device_pixel(tx as i16, ty as i16));
            let result = raster_op(op, sp, dp);
            backend.put_device_pixel(tx as i16, ty as i16, backend.rgb565_to_device(result));
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

/// Number of `u32` words a `bb_save` buffer for a `w × h` screen region
/// needs. The save bitmap is **16-bit RGB565 chunky** (one `u16` per pixel,
/// rows padded to 16 px), unified with the app-visible memory-MFDB
/// wireformat and EmuTOS' own truecolor `bb_save` buffer (`gemgsxif.c:334`,
/// `gl_nplanes = v_planes = 16`). The AES form/menu code allocates it as a
/// `Vec<u32>` — the container element type is an allocation detail while the
/// *pixels* are 16-bit (`docs/COMPATIBILITY.md`): two 16-bit pixels per
/// `u32`, so the count is `stride_px * h / 2 = fd_wdwidth * 8 * h`.
/// `stride_px` is a multiple of 16, so the division is exact. AES allocates
/// exactly this from the kernel heap.
pub fn bb_save_words(w: i16, h: i16) -> usize {
    (mem_wdwidth(w) as usize) * 8 * (h.max(0) as usize)
}

/// AES `bb_save` (EmuTOS `gemgsxif.c` `bb_set`, save direction): copy the
/// screen rectangle `(x, y, w, h)` into the caller-owned buffer `mem` as a
/// 16-bit RGB565 device-format bitmap (a Screen→Memory `vro_cpyfm`, `S_ONLY`
/// / replace). Since the screen is 5-bit-quantised, the RGB565 store is
/// lossless and the matching [`bb_restore`] reproduces the pixels bit-for-
/// bit. Called directly by the AES form manager rather than through a
/// re-entrant trap. `mem` is a `u32` pointer for the caller's convenience;
/// its bytes hold two 16-bit pixels per word (see [`bb_save_words`]).
///
/// **The mouse cursor is hidden for the copy** (`bb_set`'s `gsx_moff()` …
/// `gsx_mon()` bracket, `gemgsxif.c:367,378`), so the save-under sprite is
/// never read into the image — otherwise the restore would paint it back as
/// background, a ghost arrow at the position the pointer had when the
/// alert opened. The bracket sits here, under `bb_save`/`bb_restore`, not in
/// the caller: `fm_alert` (`gemfmalt.c:363,390`) has none of its own. The
/// hide depth counts (EmuTOS `gl_moff`), so a caller's own bracket nests.
///
/// # Safety
/// `mem` must point at a buffer of at least [`bb_save_words`]`(w, h)`
/// `u32` words that stays alive until the matching [`bb_restore`].
pub unsafe fn bb_save(backend: &dyn RasterBackend, mem: *mut u32, x: i16, y: i16, w: i16, h: i16) {
    let dst = Mfdb::device_mem(mem as u64, w, h);
    super::mouse::hide(backend);
    screen_to_mem(backend, &dst, Rect { x, y, w, h }, 0, 0, OP_REPLACE);
    super::mouse::show(backend);
}

/// AES `bb_restore`: copy the 16-bit RGB565 device-format buffer `mem`
/// (filled by [`bb_save`]) back to the screen rectangle `(x, y, w, h)` — a
/// Memory→Screen `vro_cpyfm` replace, clipped to `clip`. The cursor is
/// hidden for the copy, as in [`bb_save`] (`bb_set`, `gemgsxif.c:367,378`):
/// the sprite's save-under is released before the screen changes under it
/// and re-taken from the restored pixels afterwards.
///
/// # Safety
/// As [`bb_save`]; `mem` must hold [`bb_save_words`]`(w, h)` `u32` words.
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
    super::mouse::hide(backend);
    mem_to_screen(backend, &src, Rect { x: 0, y: 0, w, h }, x, y, OP_REPLACE, clip);
    super::mouse::show(backend);
}

/// Truecolor `vr_trnfm` branch — a faithful port of EmuTOS `vr_trnfm16`
/// (`vdi_raster.c:190-256`). Converts a bitmap between **16-bit standard**
/// form (`fd_stand = 1`: 16 plane-sequential planes, machine-independent)
/// and **Truecolor device-dependent** form (`fd_stand = 0`: 16-bit
/// RGB565-chunky, one `UWORD` per pixel — NewTOS' memory-MFDB wireformat,
/// [`CHUNKY_PLANES`]). Direction is chosen by `src.stand`; both forms hold
/// the same `planesize * 16` words (`planesize = fd_h * fd_wdwidth`, in
/// words) laid out flat, so the transpose is pure index arithmetic — a
/// standard plane `p` contributes pixel-word bit `0x8000 >> p`, and pixel
/// word `k` maps to plane word `k / 16` bit `0x8000 >> (k & 15)`. With the
/// unified 16-bit memory format (TC3b) there is no format jump: the chunky
/// output is exactly what [`vro_cpyfm`] / [`bb_save`] read, resolving
/// AUD-V5-002/003.
///
/// The destination `fd_stand` is **not** written here, matching EmuTOS'
/// truecolor branch (which, unlike the ≤8-plane planar path below, leaves
/// `dst_mfdb->fd_stand` untouched — the caller stamps the target form).
/// Works in place via a zeroed scratch buffer (EmuTOS `dos_alloc_anyram`);
/// a tight heap fails silently, exactly as the reference does.
fn vr_trnfm16(src: &Mfdb, dst: &Mfdb) {
    // `planesize = fd_h * fd_wdwidth` words per plane; the whole form (either
    // direction) is `planesize * 16` words (`formsize = planesize * 2 * 16`
    // bytes, `vdi_raster.c:198-199`).
    let planesize = (src.h.max(0) as usize) * (src.wdwidth.max(0) as usize);
    if planesize == 0 {
        return;
    }
    let total = planesize * 16;

    let s = src.addr as *const u16;
    let inplace = src.addr == dst.addr;

    // Destination buffer: the caller's `dst` (cleared), or a zeroed scratch
    // when transforming in place (EmuTOS `dos_alloc_anyram` + `bzero`). A
    // failed allocation is a silent no-op, matching the reference.
    let scratch = if inplace {
        let Ok(layout) = Layout::array::<u16>(total) else {
            return;
        };
        // SAFETY: `total >= 16` (planesize >= 1), so the layout is non-zero.
        let p = unsafe { alloc_zeroed(layout) } as *mut u16;
        if p.is_null() {
            return;
        }
        Some((p, layout))
    } else {
        None
    };
    let d = match &scratch {
        Some((p, _)) => *p,
        None => {
            // EmuTOS `bzero(dst, formsize)` — clear the output before OR-ing.
            // SAFETY: the dst MFDB describes `total` words at `dst.addr`.
            unsafe { core::ptr::write_bytes(dst.addr as *mut u16, 0, total) };
            dst.addr as *mut u16
        }
    };

    // SAFETY: `s` holds `total` words (the source form) and `d` holds `total`
    // words (the caller's dst or the `total`-word scratch); every index below
    // stays within `[0, total)`, mirroring the reference loops 1:1. In the
    // in-place case `s` and `d` are disjoint (scratch), so there is no alias.
    unsafe {
        if src.stand != 0 {
            // standard (16 plane-sequential planes) -> device (chunky):
            // plane `p` sets bit `0x8000 >> p` of each pixel word.
            let mut src_idx = 0usize;
            for p in 0..16u32 {
                let dst_mask = 0x8000u16 >> p;
                let mut work = 0usize;
                for _ in 0..planesize {
                    let sword = *s.add(src_idx);
                    src_idx += 1;
                    let mut src_mask = 0x8000u16;
                    while src_mask != 0 {
                        if sword & src_mask != 0 {
                            *d.add(work) |= dst_mask;
                        }
                        work += 1;
                        src_mask >>= 1;
                    }
                }
            }
        } else {
            // device (chunky) -> standard (16 plane-sequential planes):
            // pixel-word bit `0x8000 >> p` becomes plane `p`.
            let mut dst_idx = 0usize;
            for p in 0..16u32 {
                let src_mask = 0x8000u16 >> p;
                let mut work = 0usize;
                for _ in 0..planesize {
                    let mut dst_mask = 0x8000u16;
                    while dst_mask != 0 {
                        if *s.add(work) & src_mask != 0 {
                            *d.add(dst_idx) |= dst_mask;
                        }
                        work += 1;
                        dst_mask >>= 1;
                    }
                    dst_idx += 1;
                }
            }
        }
    }

    // In place: copy the finished scratch back over the shared buffer, free.
    if let Some((p, layout)) = scratch {
        // SAFETY: `p` holds `total` words (`alloc_zeroed(layout)`); `dst.addr`
        // holds `total` words (MFDB geometry). Distinct allocations.
        unsafe {
            core::ptr::copy_nonoverlapping(p as *const u16, dst.addr as *mut u16, total);
            dealloc(p as *mut u8, layout);
        }
    }
}

/// Opcode 110 — `vr_trnfm`: transform a bitmap between **standard**
/// (plane-sequential, machine-independent) and **device-dependent**
/// (word-interleaved for 1-8 planes, RGB565-chunky for Truecolor) form.
/// The destination's `fd_stand` is forced to the opposite of the source's
/// (planar path only); works in place when the addresses coincide. Faithful
/// port of EmuTOS `vdi_vr_trnfm` (`vdi_raster.c:269-356`), including the
/// Truecolor gate (`:281-294`).
pub fn vr_trnfm(pb: &Vdipb, backend: &dyn RasterBackend) {
    let abi = pb.abi();
    // SAFETY: caller-ABI MFDB-pointer convention (see module docs).
    let src = unsafe { Mfdb::from_ptr(pb.contrl_ptr(0), abi) };
    let dst_ptr = pb.contrl_ptr(1);
    if src.addr == 0 || dst_ptr == 0 {
        return;
    }
    // SAFETY: dst MFDB has `abi`'s layout (addr@0, …, fd_stand at
    // `stand_off`).
    let dst = unsafe { Mfdb::from_ptr(dst_ptr, abi) };
    if dst.addr == 0 {
        return;
    }

    // TRUECOLOR_MODE branch (EmuTOS `vdi_vr_trnfm`, `vdi_raster.c:281-294`):
    // in a Truecolor mode (`TRUECOLOR_MODE = v_planes > 8`) a source with more
    // than 8 planes is transformed between 16-bit standard and Truecolor
    // device form, *not* by the planar interleave below. The backend reports
    // `v_planes` (16 under Falcon truecolor); a non-truecolor backend
    // (`planes() <= 8`) falls through to the planar path, exactly as EmuTOS'
    // gate does. (AUD-V5-002/003.)
    if backend.planes() > 8 && src.nplanes > 8 {
        vr_trnfm16(&src, &dst);
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

    // Force the destination form opposite to the source. `fd_stand` sits
    // after `fd_addr` + three WORDs, so its byte offset shifts with the
    // ABI: 14 for LP64 (8B ptr), 10 for ILP32 (4B ptr).
    let new_stand: i16 = if src.stand != 0 { 0 } else { 1 };
    let stand_off = match abi {
        Abi::Lp64 => 14,
        Abi::Ilp32 => 10,
    };
    // SAFETY: dst MFDB is valid; fd_stand is the WORD at `stand_off`.
    unsafe { ((dst_ptr as *mut u8).add(stand_off) as *mut i16).write_unaligned(new_stand) };

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
