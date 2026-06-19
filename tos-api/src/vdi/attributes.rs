//! VDI attribute setters needed for the first drawing primitives.
//!
//! Spec: `docs/api/vdi/attributes/`. Each validates its input into the
//! [`Vwk`](super::vwk::Vwk) and echoes the accepted value; none can
//! fail (no VDI error return). Only the setters the bring-up demo uses
//! are here (line/fill colour + fill interior); the rest land with the
//! opcodes that consume them.

use super::backend::RasterBackend;
use super::vwk::{validate_color_index, Vwk};
use super::Vdipb;

/// Writing-mode bounds (1-based API), EmuTOS `vdi_control.c`.
const MIN_WRT_MODE: i16 = 1;
const MAX_WRT_MODE: i16 = 4;
const DEF_WRT_MODE: i16 = 1; // replace

/// Opcode 32 — `vswr_mode`: set the writing/raster mode. `INTIN[0]` is
/// 1-based (1 replace, 2 transparent, 3 XOR, 4 reverse-transparent);
/// stored 0-based in `wrt_mode`. Out-of-range defaults to replace.
pub fn vswr_mode(pb: &Vdipb, vwk: &mut Vwk) {
    let mut mode = pb.intin(0);
    if !(MIN_WRT_MODE..=MAX_WRT_MODE).contains(&mode) {
        mode = DEF_WRT_MODE;
    }
    vwk.wrt_mode = mode - 1;
    pb.set_intout(0, mode);
    pb.set_contrl(4, 1);
}

/// Opcode 129 — `vs_clip`: set the clipping rectangle. `INTIN[0]` is the
/// enable flag; when set, `PTSIN[0..3]` give the rectangle (corners
/// normalised, then intersected with the screen). When clear, the clip
/// is reset to the full device. No output.
pub fn vs_clip(pb: &Vdipb, vwk: &mut Vwk, backend: &dyn RasterBackend) {
    let xres = backend.width() as i16 - 1;
    let yres = backend.height() as i16 - 1;
    vwk.clip = pb.intin(0) != 0;
    if vwk.clip {
        let mut x1 = pb.ptsin(0);
        let mut y1 = pb.ptsin(1);
        let mut x2 = pb.ptsin(2);
        let mut y2 = pb.ptsin(3);
        if x1 > x2 {
            core::mem::swap(&mut x1, &mut x2);
        }
        if y1 > y2 {
            core::mem::swap(&mut y1, &mut y2);
        }
        vwk.xmn_clip = x1.max(0);
        vwk.ymn_clip = y1.max(0);
        vwk.xmx_clip = x2.min(xres);
        vwk.ymx_clip = y2.min(yres);
    } else {
        vwk.xmn_clip = 0;
        vwk.ymn_clip = 0;
        vwk.xmx_clip = xres;
        vwk.ymx_clip = yres;
    }
}

/// Opcode 17 — `vsl_color`: set the polyline colour.
pub fn vsl_color(pb: &Vdipb, vwk: &mut Vwk, ncol: u16) {
    let c = validate_color_index(pb.intin(0), ncol);
    vwk.line_color = c;
    pb.set_intout(0, c as i16);
    pb.set_contrl(4, 1);
}

/// Opcode 25 — `vsf_color`: set the fill-area colour.
pub fn vsf_color(pb: &Vdipb, vwk: &mut Vwk, ncol: u16) {
    let c = validate_color_index(pb.intin(0), ncol);
    vwk.fill_color = c;
    pb.set_intout(0, c as i16);
    pb.set_contrl(4, 1);
}

/// Opcode 22 — `vst_color`: set the text colour.
pub fn vst_color(pb: &Vdipb, vwk: &mut Vwk, ncol: u16) {
    let c = validate_color_index(pb.intin(0), ncol);
    vwk.text_color = c;
    pb.set_intout(0, c as i16);
    pb.set_contrl(4, 1);
}

/// Opcode 39 — `vst_alignment`: set the text alignment.
/// `INTIN[0]` = horizontal (0 left, 1 centre, 2 right), `INTIN[1]` =
/// vertical (0 baseline … 5 top). Out-of-range clamps to 0.
pub fn vst_alignment(pb: &Vdipb, vwk: &mut Vwk) {
    let h = pb.intin(0);
    let v = pb.intin(1);
    vwk.h_align = if (0..=2).contains(&h) { h } else { 0 };
    vwk.v_align = if (0..=5).contains(&v) { v } else { 0 };
    pb.set_intout(0, vwk.h_align);
    pb.set_intout(1, vwk.v_align);
    pb.set_contrl(4, 2);
}

/// Opcode 23 — `vsf_interior`: set the fill interior style
/// (0 = hollow, 1 = solid, 2 = pattern, 3 = hatch, 4 = user).
pub fn vsf_interior(pb: &Vdipb, vwk: &mut Vwk) {
    let s = pb.intin(0);
    vwk.fill_style = if (0..=4).contains(&s) { s } else { 0 };
    pb.set_intout(0, vwk.fill_style);
    pb.set_contrl(4, 1);
}

const FIS_PATTERN: i16 = 2;
const MAX_FILL_PATTERN: i16 = 24;
const MAX_FILL_HATCH: i16 = 12;

/// Opcode 24 — `vsf_style`: set the pattern/hatch index (1-based in the
/// API), validated against the current interior style's range. Stored
/// 0-based in `fill_index`.
pub fn vsf_style(pb: &Vdipb, vwk: &mut Vwk) {
    let mut fi = pb.intin(0);
    let max = if vwk.fill_style == FIS_PATTERN {
        MAX_FILL_PATTERN
    } else {
        MAX_FILL_HATCH
    };
    if !(1..=max).contains(&fi) {
        fi = 1; // default
    }
    vwk.fill_index = fi - 1;
    pb.set_intout(0, fi);
    pb.set_contrl(4, 1);
}

/// Opcode 112 — `vsf_udpat`: set the user-defined fill pattern from
/// `INTIN` (`CONTRL[3]` words). A 16-word count is a single-plane
/// pattern; a multiple of 16 is multi-plane (we store plane 0). Invalid
/// counts are ignored.
pub fn vsf_udpat(pb: &Vdipb, vwk: &mut Vwk) {
    let count = pb.contrl(3);
    if count < 16 || count % 16 != 0 {
        return;
    }
    for i in 0..16 {
        vwk.ud_patrn[i] = pb.intin(i) as u16;
    }
    vwk.multifill = count != 16;
}
