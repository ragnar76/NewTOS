//! VDI attribute setters needed for the first drawing primitives.
//!
//! Spec: `docs/api/vdi/attributes/`. Each validates its input into the
//! [`Vwk`](super::vwk::Vwk) and echoes the accepted value; none can
//! fail (no VDI error return). Only the setters the bring-up demo uses
//! are here (line/fill colour + fill interior); the rest land with the
//! opcodes that consume them.

use super::backend::RasterBackend;
use super::font;
use super::req_col;
use super::vwk::{validate_color_index, Vwk};
use super::Vdipb;

// Line / marker / text attribute bounds (EmuTOS `vdi_defs.h`).
const MAX_LINE_STYLE: i16 = 7;
const MAX_LINE_WIDTH: i16 = 40; // SIZ_TAB[6]
const MIN_END_STYLE: i16 = 0; // squared
const MAX_END_STYLE: i16 = 2; // round
const MIN_MARK_STYLE: i16 = 1;
const MAX_MARK_STYLE: i16 = 6;
const DEF_MKWD: i16 = 15; // SIZ_TAB[8] base marker width
const DEF_MKHT: i16 = 11; // SIZ_TAB[9] base marker height
const MAX_MKHT: i16 = 88; // SIZ_TAB[11]
const EFFECTS_MASK: i16 = 0x1F; // INQ_TAB[2] supported text effects

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

/// Opcode 15 — `vsl_type`: set the line style (1..MAX, else solid).
/// Stored 0-based in `line_index`.
pub fn vsl_type(pb: &Vdipb, vwk: &mut Vwk) {
    let v = pb.intin(0);
    vwk.line_index = if (1..=MAX_LINE_STYLE).contains(&v) { v - 1 } else { 0 };
    pb.set_intout(0, vwk.line_index + 1);
    pb.set_contrl(4, 1);
}

/// Opcode 16 — `vsl_width`: set the line width from `PTSIN[0]`. Clamped
/// to `[1, MAX_LINE_WIDTH]` and forced **odd** (the line algorithm
/// centres on a pixel). Echoes the width in `PTSOUT[0]`.
pub fn vsl_width(pb: &Vdipb, vwk: &mut Vwk) {
    let mut w = pb.ptsin(0).clamp(1, MAX_LINE_WIDTH);
    if w % 2 == 0 {
        w -= 1; // force odd
    }
    vwk.line_width = w;
    pb.set_ptsout(0, w);
    pb.set_ptsout(1, 0);
    pb.set_contrl(2, 1);
}

/// Opcode 108 — `vsl_ends`: set the line end styles (begin = `INTIN[0]`,
/// end = `INTIN[1]`; 0 squared / 1 arrow / 2 round, else squared).
pub fn vsl_ends(pb: &Vdipb, vwk: &mut Vwk) {
    let validate = |v: i16| if (MIN_END_STYLE..=MAX_END_STYLE).contains(&v) { v } else { MIN_END_STYLE };
    vwk.line_beg = validate(pb.intin(0));
    vwk.line_end = validate(pb.intin(1));
    pb.set_intout(0, vwk.line_beg);
    pb.set_intout(1, vwk.line_end);
    pb.set_contrl(4, 2);
}

/// Opcode 113 — `vsl_udsty`: set the 16-bit user-defined line-style mask
/// (no validation). No output.
pub fn vsl_udsty(pb: &Vdipb, vwk: &mut Vwk) {
    vwk.ud_ls = pb.intin(0) as u16;
}

/// Opcode 18 — `vsm_type`: set the marker type (1..6, else asterisk).
/// Stored 0-based in `mark_index`.
pub fn vsm_type(pb: &Vdipb, vwk: &mut Vwk) {
    let v = pb.intin(0);
    vwk.mark_index = if (MIN_MARK_STYLE..=MAX_MARK_STYLE).contains(&v) { v - 1 } else { 2 };
    pb.set_intout(0, vwk.mark_index + 1);
    pb.set_contrl(4, 1);
}

/// Opcode 19 — `vsm_height`: set the marker height from `PTSIN[1]`.
/// Clamped to `[DEF_MKHT, MAX_MKHT]`; derives an integer `mark_scale` of
/// the base marker. Echoes the actual marker width/height in `PTSOUT`.
pub fn vsm_height(pb: &Vdipb, vwk: &mut Vwk) {
    let h = pb.ptsin(1).clamp(DEF_MKHT, MAX_MKHT);
    vwk.mark_height = h;
    let scale = (h + DEF_MKHT / 2) / DEF_MKHT;
    vwk.mark_scale = scale;
    pb.set_ptsout(0, scale * DEF_MKWD);
    pb.set_ptsout(1, scale * DEF_MKHT);
    pb.set_contrl(2, 1);
}

/// Opcode 20 — `vsm_color`: set the marker colour.
pub fn vsm_color(pb: &Vdipb, vwk: &mut Vwk, ncol: u16) {
    let c = validate_color_index(pb.intin(0), ncol);
    vwk.mark_color = c;
    pb.set_intout(0, c as i16);
    pb.set_contrl(4, 1);
}

/// Opcode 106 — `vst_effects`: set the text effect bits, masked to the
/// supported set (`0x1F`: bold/light/italic/underline/outline).
pub fn vst_effects(pb: &Vdipb, vwk: &mut Vwk) {
    vwk.style = pb.intin(0) & EFFECTS_MASK;
    pb.set_intout(0, vwk.style);
    pb.set_contrl(4, 1);
}

/// Opcode 13 — `vst_rotation`: set the text baseline angle in 1/10°.
/// The angle is normalised to `[0,3599]`, then snapped to the nearest
/// right angle (`chup` ∈ {0,900,1800,2700,**3600**}). EmuTOS
/// `vdi_text.c:924-936`: an angle in 3150..3599 rounds up to 3600 (kept
/// distinct from 0 — no `% 3600`).
pub fn vst_rotation(pb: &Vdipb, vwk: &mut Vwk) {
    let mut angle = pb.intin(0) % 3600;
    if angle < 0 {
        angle += 3600;
    }
    vwk.chup = ((angle + 450) / 900) * 900;
    pb.set_intout(0, vwk.chup);
    pb.set_contrl(4, 1);
}

/// Opcode 21 — `vst_font`: select a text face. Only the system font
/// (face 1) exists, so every request resolves to it.
pub fn vst_font(pb: &Vdipb, _vwk: &mut Vwk) {
    pb.set_intout(0, font::FONT_ID);
    pb.set_contrl(4, 1);
}

/// Fill `PTSOUT[0..3]` with the actual character/cell metrics of the
/// single system font (EmuTOS `setup_width_height`).
fn report_font_size(pb: &Vdipb) {
    pb.set_ptsout(0, font::MAX_CHAR_WIDTH); // char width  (7)
    pb.set_ptsout(1, font::TOP); // char height (top = 13)
    pb.set_ptsout(2, font::CELL_W); // cell width  (8)
    pb.set_ptsout(3, font::CELL_H); // cell height (16)
    pb.set_contrl(2, 2);
}

/// Opcode 12 — `vst_height`: request a text height in device units.
/// Only the one fixed bitmap face exists (no scaling), so the actual
/// 8×16 metrics are reported regardless of the request.
pub fn vst_height(pb: &Vdipb, _vwk: &mut Vwk) {
    report_font_size(pb);
}

/// Opcode 107 — `vst_point`: request a text height in points. Reports
/// the single available point size (10) and its metrics.
pub fn vst_point(pb: &Vdipb, _vwk: &mut Vwk) {
    report_font_size(pb);
    pb.set_intout(0, font::POINT);
    pb.set_contrl(4, 1);
}

/// Opcode 119 — `vst_load_fonts`: load extra font faces. Only the
/// built-in system font exists, so no additional faces are ever loaded;
/// returns the count of newly loaded fonts (0).
pub fn vst_load_fonts(pb: &Vdipb) {
    pb.set_intout(0, 0);
    pb.set_contrl(4, 1);
}

/// Opcode 120 — `vst_unload_fonts`: unload extra fonts. Nothing was ever
/// loaded beyond the system font, so this is a no-op.
pub fn vst_unload_fonts(_pb: &Vdipb) {}

/// Opcode 104 — `vsf_perimeter`: enable/disable the fill-area outline.
pub fn vsf_perimeter(pb: &Vdipb, vwk: &mut Vwk) {
    vwk.fill_per = pb.intin(0) != 0;
    pb.set_intout(0, if vwk.fill_per { 1 } else { 0 });
    pb.set_contrl(4, 1);
}

/// Opcode 14 — `vs_color`: set a palette entry. `INTIN[0]` = colour
/// index, `INTIN[1..3]` = R/G/B. Out-of-range indices are ignored. The
/// **raw** request is stored in the `REQ_COL` shadow (so `vq_color`
/// flag 0 round-trips it verbatim, possibly outside 0..1000); only the
/// copy programmed into the CLUT is clamped to 0..=1000. No output.
/// EmuTOS `vdi_col.c:656-704`.
pub fn vs_color(pb: &Vdipb, ncol: u16, backend: &dyn RasterBackend) {
    let idx = pb.intin(0);
    if idx < 0 || idx as u16 >= ncol {
        return;
    }
    let (r, g, b) = (pb.intin(1), pb.intin(2), pb.intin(3));
    // Store the raw, unclamped request (EmuTOS `REQ_COL[colnum][i] = *intin`).
    // SAFETY: single trap path (see `req_col`).
    unsafe { req_col::store_request(idx as u16, r, g, b) };
    // Clamp only the DAC copy to the VDI 0..1000 range.
    let clamp = |v: i16| v.clamp(0, 1000) as u16;
    backend.set_color(idx as u16, clamp(r), clamp(g), clamp(b));
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
/// `INTIN` (`CONTRL[3]` words). EmuTOS (`vdi_fill.c:149-166`) accepts
/// **only** a 16-word single-plane pattern (`multifill = 0`) or a full
/// `planes*16`-word multi-plane pattern (`multifill = 1`, `planes =
/// INQ_TAB[4] = log2(ncol)`); any other count is invalid and leaves the
/// state unchanged. We store plane 0 either way (multi-plane colour
/// reduction — an accepted single-plane-backend limitation).
pub fn vsf_udpat(pb: &Vdipb, vwk: &mut Vwk, ncol: u16) {
    let count = pb.contrl(3);
    let planes = ncol.trailing_zeros() as i16; // INQ_TAB[4]
    if count == 16 {
        vwk.multifill = false;
    } else if count == planes * 16 {
        vwk.multifill = true;
    } else {
        return; // invalid count: no state change
    }
    for i in 0..16 {
        vwk.ud_patrn[i] = pb.intin(i) as u16;
    }
}
