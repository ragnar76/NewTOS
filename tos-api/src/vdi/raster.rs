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
//! Bring-up scope: `vrt_cpyfm` with a 1-plane source (icons/sprites),
//! replace + transparent modes; `vro_cpyfm` for the screen-to-screen
//! case (replace / XOR). Off-screen colour copies and the full 16-op /
//! multi-plane paths land later.

use super::backend::{MonoSrc, RasterBackend, Rect};
use super::vwk::{validate_color_index, Vwk};
use super::Vdipb;

/// Memory Form Definition Block (fixed ABI; native pointer in `addr`).
/// `w`/`stand` are parsed for completeness but unused by the bring-up
/// blit (extent comes from `PTSIN`, stride from `wdwidth`); they feed
/// `vr_trnfm` / validation later.
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

    fn is_screen(&self) -> bool {
        self.addr == 0
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

    let mode = pb.intin(0);
    let opaque = match mode {
        2 => true,  // MD_REPLACE: fg on 1, bg on 0
        1 => false, // MD_TRANS: fg on 1, leave 0
        _ => false, // XOR/ERASE not yet distinguished → transparent
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
        opaque,
        vwk.clip_rect(),
    );
}

/// Opcode 109 — `vro_cpyfm`: opaque copy. Bring-up handles the
/// screen-to-screen case (both MFDBs are the screen) with the BitBlt
/// logic op in `INTIN[0]`.
pub fn vro_cpyfm(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    // SAFETY: native MFDB-pointer convention (see module docs).
    let (src, dst) = unsafe {
        (Mfdb::from_ptr(pb.contrl_u64(7)), Mfdb::from_ptr(pb.contrl_u64(11)))
    };
    if !src.is_screen() || !dst.is_screen() {
        return; // only screen↔screen for now
    }
    let mode = pb.intin(0);
    if mode & !0x1F != 0 {
        return; // invalid logic op
    }
    let s = rect_from_ptsin(pb, 0);
    let d = rect_from_ptsin(pb, 4);
    backend.copy_rect(s, d.x, d.y, mode as u8, vwk.clip_rect());
}
