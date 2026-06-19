//! VDI text output — `v_gtext` (opcode 8) and the GDP `v_justified`
//! (opcode 11 / sub-op 10). Spec: `docs/api/vdi/output/_text.md`,
//! `docs/api/vdi/output/_gdp.md`.
//!
//! The single monospace 8×16 system [`font`], one glyph per `INTIN`
//! character code, with horizontal/vertical alignment and the text
//! effects (thicken/light/skew/underline) from `style`. Glyphs blit
//! transparently in `text_color`; rotation is limited to the four right
//! angles (only `chup == 0` is drawn horizontally here).

use super::backend::{ClipRect, RasterBackend};
use super::font;
use super::vwk::Vwk;
use super::Vdipb;

// Text effect bits (`vst_effects`).
const T_THICKEN: i16 = 0x01;
const T_LIGHT: i16 = 0x02;
const T_SKEW: i16 = 0x04;
const T_UNDERLINE: i16 = 0x08;

/// Transform one glyph row for the light (grey) and skew (italic)
/// effects. `row` is 0..15 from the cell top.
fn effect_row(bits: u8, row: usize, style: i16) -> (u8, i16) {
    let mut b = bits;
    if style & T_LIGHT != 0 {
        // Checkerboard mask drops half the pixels for a greyed look.
        b &= if row & 1 == 0 { font::LIGHTEN } else { !font::LIGHTEN };
    }
    // Skew leans the top of the cell right; up to ~3 px over 16 rows.
    let shift = if style & T_SKEW != 0 { (15 - row as i16) / 5 } else { 0 };
    (b, shift)
}

/// Blit one cell at `(x, y)` applying the active text effects.
fn draw_cell(backend: &dyn RasterBackend, x: i16, y: i16, code: u16, vwk: &Vwk, clip: ClipRect) {
    let glyph = font::glyph(code);
    let style = vwk.style;
    let mut cell = [0u8; 16];
    for (row, out) in cell.iter_mut().enumerate() {
        let (b, shift) = effect_row(glyph[row], row, style);
        *out = b >> shift.clamp(0, 7);
    }
    backend.draw_glyph(x, y, &cell, vwk.text_color, clip);
    if style & T_THICKEN != 0 {
        // Bold: overstrike one pixel to the right.
        backend.draw_glyph(x + font::THICKEN, y, &cell, vwk.text_color, clip);
    }
}

/// Vertical-alignment reference → glyph-cell top, for `v_align`.
fn top_for(y: i16, v_align: i16) -> i16 {
    match v_align {
        0 => y - font::TOP,                  // baseline
        1 => y - (font::TOP - font::HALF),   // half line
        2 => y - (font::TOP - font::ASCENT), // ascent (cap top)
        3 => y - (font::CELL_H - 1),         // bottom of cell
        4 => y - (font::TOP + font::DESCENT),// descent line
        _ => y,                              // top of cell (5)
    }
}

/// Draw the underline run if the effect is set: a `UL_SIZE`-thick line
/// just below the baseline, spanning `[x0, x0+width)`.
fn underline(backend: &dyn RasterBackend, x0: i16, top_y: i16, width: i16, vwk: &Vwk, clip: ClipRect) {
    if vwk.style & T_UNDERLINE == 0 || width <= 0 {
        return;
    }
    let y = top_y + font::TOP + 1; // one row under the baseline
    for r in 0..font::UL_SIZE {
        backend.draw_line(x0, y + r, x0 + width - 1, y + r, vwk.text_color, clip, 0xFFFF);
    }
}

/// Opcode 8 — `v_gtext`: draw the `CONTRL[3]` characters in `INTIN`
/// (one code per WORD) at `PTSIN[0..1]`, in the current text colour,
/// alignment and effects.
pub fn v_gtext(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let count = pb.contrl(3);
    if count <= 0 {
        return;
    }
    let (x, y) = (pb.ptsin(0), pb.ptsin(1));

    let width = count * font::CELL_W;
    let start_x = match vwk.h_align {
        1 => x - width / 2, // centre
        2 => x - width,     // right
        _ => x,             // left (0)
    };
    let top_y = top_for(y, vwk.v_align);

    let clip = vwk.clip_rect();
    let mut cx = start_x;
    for i in 0..count as usize {
        draw_cell(backend, cx, top_y, pb.intin(i) as u16, vwk, clip);
        cx += font::CELL_W;
    }
    underline(backend, start_x, top_y, width, vwk, clip);
}

/// GDP sub-op 10 — `v_justified`: draw a run of text spread to fill a
/// target width. `INTIN[0]` = interword flag, `INTIN[1]` = interchar
/// flag, `INTIN[2..]` = char codes (`CONTRL[3] - 2` of them).
/// `PTSIN[0..1]` = position, `PTSIN[2]` = target width. Slack is shared
/// across word gaps and/or all characters. Drawn horizontally (`chup`
/// right-angle rotations beyond 0 fall back to left-to-right).
pub fn v_justified(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let nchars = (pb.contrl(3) - 2).max(0) as usize;
    if nchars == 0 {
        return;
    }
    let interword = pb.intin(0) != 0;
    let interchar = pb.intin(1) != 0;
    let x0 = pb.ptsin(0);
    let top_y = top_for(pb.ptsin(1), vwk.v_align);
    let max_x = pb.ptsin(2);

    // Natural run width and the word-gap count (spaces).
    let natural = nchars as i16 * font::CELL_W;
    let spaces = (0..nchars).filter(|&i| pb.intin(2 + i) as u16 == b' ' as u16).count() as i16;
    let mut slack = max_x - natural;

    // Per-gap interword extra (clamped to ±½ cell), per-char interchar extra.
    let word_extra = if interword && spaces > 0 {
        (slack / spaces).clamp(-(font::CELL_W / 2), font::CELL_W / 2)
    } else {
        0
    };
    slack -= word_extra * spaces;
    let char_extra = if interchar && nchars > 1 { slack / (nchars as i16 - 1) } else { 0 };

    let clip = vwk.clip_rect();
    let mut cx = x0;
    for i in 0..nchars {
        let code = pb.intin(2 + i) as u16;
        draw_cell(backend, cx, top_y, code, vwk, clip);
        cx += font::CELL_W + char_extra;
        if code == b' ' as u16 {
            cx += word_extra;
        }
    }
    underline(backend, x0, top_y, cx - x0, vwk, clip);
}
