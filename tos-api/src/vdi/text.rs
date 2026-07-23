//! VDI text output — `v_gtext` (opcode 8) and the GDP `v_justified`
//! (opcode 11 / sub-op 10). Spec: `docs/api/vdi/output/_text.md`,
//! `docs/api/vdi/output/_gdp.md`.
//!
//! Renders the workstation's currently-selected system font
//! ([`Vwk::cur_font`](super::vwk::Vwk) — 6×6, 8×8 or 8×16, chosen by
//! `vst_height`/`vst_point`), one glyph per `INTIN` character code, with
//! horizontal/vertical alignment and the text effects
//! (thicken/light/skew/underline) from `style`. Glyphs blit
//! transparently in `text_color`; rotation is limited to the four right
//! angles (only `chup == 0` is drawn horizontally here).

use super::backend::{ClipRect, RasterBackend, WriteMode};
use super::font::{self, SysFont};
use super::vwk::Vwk;
use super::Vdipb;

// Text effect bits (`vst_effects`).
const T_THICKEN: i16 = 0x01;
const T_LIGHT: i16 = 0x02;
const T_SKEW: i16 = 0x04;
const T_UNDERLINE: i16 = 0x08;

/// Transform one glyph row for the light (grey) and skew (italic)
/// effects. `row` is 0-based from the cell top; `cell_h` is the font's
/// `form_height` (16 for the 8×16 font — the previous fixed value).
fn effect_row(f: &SysFont, bits: u8, row: usize, cell_h: i16, style: i16) -> (u8, i16) {
    let mut b = bits;
    if style & T_LIGHT != 0 {
        // Checkerboard mask drops half the pixels for a greyed look
        // (font `lighten`, low byte — 0x55 for all three system fonts).
        let mask = (f.lighten & 0xFF) as u8;
        b &= if row & 1 == 0 { mask } else { !mask };
    }
    // Skew leans the top of the cell right (V4-006: this is the existing
    // approximation, kept — the SKEWMASK-exact skew is a later task; here it
    // is only made cell-height-aware so it stays bounded for small fonts).
    let shift = if style & T_SKEW != 0 { (cell_h - 1 - row as i16) / 5 } else { 0 };
    (b, shift)
}

/// Blit one cell at `(x, y)` in font `f`, applying the active text
/// effects, in the workstation writing mode.
fn draw_cell(backend: &dyn RasterBackend, f: &SysFont, x: i16, y: i16, code: u16, vwk: &Vwk, clip: ClipRect) {
    let style = vwk.style;
    let cell_h = f.form_height;
    let mut cell = [0u8; 16];
    for (row, out) in cell.iter_mut().take(cell_h as usize).enumerate() {
        let (b, shift) = effect_row(f, font::glyph_row(f.id, code, row), row, cell_h, style);
        let mut b = b >> shift.clamp(0, 7);
        if style & T_THICKEN != 0 {
            // Bold: overstrike one pixel to the right, baked into the same
            // source cell (EmuTOS ORs a right-shifted copy into the source
            // before the write-mode blit). Baking it — rather than blitting
            // the glyph twice — keeps a single source bitmap, so the four
            // write modes (notably replace/xor, where a second blit would
            // re-clear or re-invert the cell) stay correct.
            b |= b >> f.thicken;
        }
        *out = b;
    }
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    backend.draw_glyph_sized(x, y, &cell[..cell_h as usize], f.max_cell_width, cell_h, vwk.text_color, mode, clip);
}

/// Vertical-alignment reference → glyph-cell top, for `v_align`
/// (EmuTOS `output_text` `delv`, `vdi_text.c:366-390`).
fn top_for(f: &SysFont, y: i16, v_align: i16) -> i16 {
    match v_align {
        0 => y - f.top,                    // baseline
        1 => y - (f.top - f.half),         // half line
        2 => y - (f.top - f.ascent),       // ascent (cap top)
        3 => y - (f.form_height - 1),      // bottom of cell
        4 => y - (f.top + f.descent),      // descent line
        _ => y,                            // top of cell (5)
    }
}

/// Draw the underline run if the effect is set: a `ul_size`-thick line
/// below the baseline, spanning `[x0, x0+width)`.
fn underline(backend: &dyn RasterBackend, f: &SysFont, x0: i16, top_y: i16, width: i16, vwk: &Vwk, clip: ClipRect) {
    if vwk.style & T_UNDERLINE == 0 || width <= 0 {
        return;
    }
    // EmuTOS `vdi_text.c:399-413`: the underline row within the cell is
    // `top + underline`, where `underline = ul_size + (bottom > ul_size ? 1
    // : 0)`. The 8×16 font (bottom=2 > ul_size=1) drops the line to cell row
    // 15 (`13 + 2`); the 6×6/8×8 fonts (bottom == ul_size) sit it on the
    // descent line (`top + ul_size`), keeping it inside the smaller cell.
    let ul_off = f.ul_size + if f.bottom > f.ul_size { 1 } else { 0 };
    let y = top_y + f.top + ul_off;
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    for r in 0..f.ul_size {
        backend.draw_line(x0, y + r, x0 + width - 1, y + r, vwk.text_color, mode, clip, 0xFFFF);
    }
}

/// Opcode 8 — `v_gtext`: draw the `CONTRL[3]` characters in `INTIN`
/// (one code per WORD) at `PTSIN[0..1]`, in the current text colour,
/// alignment, effects and selected system font.
pub fn v_gtext(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let count = pb.contrl(3);
    if count <= 0 {
        return;
    }
    let f = font::font(vwk.cur_font);
    let cell_w = f.max_cell_width;
    let (x, y) = (pb.ptsin(0), pb.ptsin(1));

    let width = count * cell_w;
    let start_x = match vwk.h_align {
        1 => x - width / 2, // centre
        2 => x - width,     // right
        _ => x,             // left (0)
    };
    let top_y = top_for(f, y, vwk.v_align);

    let clip = vwk.clip_rect();
    let mut cx = start_x;
    for i in 0..count as usize {
        draw_cell(backend, f, cx, top_y, pb.intin(i) as u16, vwk, clip);
        cx += cell_w;
    }
    underline(backend, f, start_x, top_y, width, vwk, clip);
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
    let f = font::font(vwk.cur_font);
    let cell_w = f.max_cell_width;
    let interword = pb.intin(0) != 0;
    let interchar = pb.intin(1) != 0;
    let x0 = pb.ptsin(0);
    let top_y = top_for(f, pb.ptsin(1), vwk.v_align);
    let max_x = pb.ptsin(2);

    // Natural run width and the word-gap count (spaces).
    let natural = nchars as i16 * cell_w;
    let spaces = (0..nchars).filter(|&i| pb.intin(2 + i) as u16 == b' ' as u16).count() as i16;
    let mut slack = max_x - natural;

    // Per-gap interword extra (clamped to ±½ cell), per-char interchar extra.
    let word_extra = if interword && spaces > 0 {
        (slack / spaces).clamp(-(cell_w / 2), cell_w / 2)
    } else {
        0
    };
    slack -= word_extra * spaces;
    let char_extra = if interchar && nchars > 1 { slack / (nchars as i16 - 1) } else { 0 };

    let clip = vwk.clip_rect();
    let mut cx = x0;
    for i in 0..nchars {
        let code = pb.intin(2 + i) as u16;
        draw_cell(backend, f, cx, top_y, code, vwk, clip);
        cx += cell_w + char_extra;
        if code == b' ' as u16 {
            cx += word_extra;
        }
    }
    underline(backend, f, x0, top_y, cx - x0, vwk, clip);
}
