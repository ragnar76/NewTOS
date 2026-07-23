//! VDI text output — `v_gtext` (opcode 8) and the GDP `v_justified`
//! (opcode 11 / sub-op 10). Spec: `docs/api/vdi/output/_text.md`,
//! `docs/api/vdi/output/_gdp.md`.
//!
//! Renders the workstation's currently-selected system font
//! ([`Vwk::cur_font`](super::vwk::Vwk) — 6×6, 8×8 or 8×16, chosen by
//! `vst_height`/`vst_point`), one glyph per `INTIN` character code, with
//! horizontal/vertical alignment and the text effects
//! (thicken/light/skew/underline/outline) from `style`. Glyphs blit
//! transparently in `text_color`; rotation is limited to the four right
//! angles (only `chup == 0` is drawn horizontally here).

use super::backend::{ClipRect, RasterBackend, WriteMode};
use super::font::{self, SysFont};
use super::vwk::Vwk;
use super::Vdipb;

// Text effect bits (`vst_effects`, EmuTOS `vdi_defs.h:118-122`).
const T_THICKEN: i16 = 0x01;
const T_LIGHT: i16 = 0x02;
const T_SKEW: i16 = 0x04;
const T_UNDERLINE: i16 = 0x08;
const T_OUTLINE: i16 = 0x10;

/// Outlining adds one pixel all around (EmuTOS `OUTLINE_THICKNESS`,
/// `vdi_defs.h:126`).
const OUTLINE_THICKNESS: i16 = 1;

/// Maximum rows of the per-glyph effect buffer: the tallest system cell
/// (16) plus the two rows outlining adds. Each row is a `u32` (leftmost
/// pixel = bit 31), which spans the widest effect cell (glyph 8 + skew 8 +
/// outline 2 = 18 px) with headroom.
const MAX_ROWS: usize = 18;

/// Per-glyph-row rightward skew shift, driven by the font's `skew`
/// bit-mask exactly as EmuTOS' `skew_msk` (`vdi_tblit.S:1279`). The blit
/// runs bottom (row `fh-1`) to top (row 0); before each scanline the mask
/// rotates left one bit and, when a 1 rolls out of bit 15, the running
/// shift bumps by one pixel. The total lean over the cell therefore equals
/// the number of set skew bits in the top `fh` bits, which the font header
/// makes `left_offset + right_offset` (8 px for the 8×16 font, 4 for 8×8,
/// 3 for 6×6) and which pivots at the baseline (shift `= left_offset`
/// there). The italic thus leans right above the baseline and left below
/// it, `left_offset` px left of the cell at the descent, `right_offset` px
/// right at the top.
fn skew_shifts(f: &SysFont, fh: usize) -> [i16; MAX_ROWS] {
    let mut shifts = [0i16; MAX_ROWS];
    let mut mask = f.skew;
    let mut shift = 0i16;
    for k in 0..fh {
        let carry = mask & 0x8000 != 0; // old bit 15 (rol.w sets carry from it)
        mask = mask.rotate_left(1);
        if carry {
            shift += 1;
        }
        shifts[fh - 1 - k] = shift; // current (bottom-up) scanline uses the bumped shift
    }
    shifts
}

/// One row of the effect buffer, or 0 for an out-of-range row index.
#[inline]
fn row_or_zero(src: &[u32; MAX_ROWS], g: i32, fh: usize) -> u32 {
    if g >= 0 && (g as usize) < fh {
        src[g as usize]
    } else {
        0
    }
}

/// Blit one cell whose glyph col 0 lands at `x_pen`, cell top at `top_y`
/// (both before any outline shift), applying the active text effects in
/// the workstation writing mode. Reimplements EmuTOS `text_blt`
/// (`vdi_textblit.c`): thicken + skew build the source, `outline()` turns
/// it into a hollow contour, and the light mask dims it during the blit.
fn draw_cell(backend: &dyn RasterBackend, f: &SysFont, x_pen: i16, top_y: i16, code: i16, vwk: &Vwk, clip: ClipRect) {
    let style = vwk.style;
    let fh = f.form_height as usize;
    let outline = style & T_OUTLINE != 0;
    let skew = style & T_SKEW != 0;
    let gcode = font::map_code(f, code);

    // 1. Build the source rows: each glyph row placed at its skew offset,
    //    with the bold overstrike (thicken) baked in. Leftmost pixel = bit 31.
    let shifts = if skew { skew_shifts(f, fh) } else { [0i16; MAX_ROWS] };
    let mut src = [0u32; MAX_ROWS];
    for (row, out) in src.iter_mut().take(fh).enumerate() {
        let mut b = font::glyph_row(f.id, gcode, row);
        if style & T_THICKEN != 0 {
            // Bold: overstrike one pixel to the right (EmuTOS ORs a
            // right-shifted copy of the source before the blit). Mono fonts
            // keep their advance (`upda_dst`), so it only fattens the glyph.
            b |= b >> f.thicken;
        }
        let s = shifts[row].clamp(0, 24) as u32;
        *out = ((b as u32) << 24) >> s;
    }

    // 2. Outline (hollow contour) or the plain source.
    let mut work = [0u32; MAX_ROWS];
    let (h, blit_x, blit_y);
    if outline {
        // Inset the glyph one pixel right so its left border fits inside the
        // u32, then dilate-and-erase: a pixel is set iff it is background yet
        // has a set 8-neighbour (EmuTOS `outline()`, `vdi_textblit.c:266-345`:
        // `result & ~curr`, hollow letters). Two extra rows hold the top and
        // bottom borders.
        let mut ins = [0u32; MAX_ROWS];
        for (dst, s) in ins.iter_mut().zip(src.iter()).take(fh) {
            *dst = *s >> OUTLINE_THICKNESS;
        }
        h = fh + 2;
        for (b, out) in work.iter_mut().take(h).enumerate() {
            let g = b as i32 - 1; // buffer row b holds glyph row b-1
            let cur = row_or_zero(&ins, g, fh);
            let above = row_or_zero(&ins, g - 1, fh);
            let below = row_or_zero(&ins, g + 1, fh);
            let neigh = (cur << 1) | (cur >> 1)
                | above | (above << 1) | (above >> 1)
                | below | (below << 1) | (below >> 1);
            *out = neigh & !cur;
        }
        blit_x = x_pen - OUTLINE_THICKNESS; // buffer col 0 sits 1 px left
        blit_y = top_y - OUTLINE_THICKNESS; // border row above the glyph
    } else {
        h = fh;
        work[..fh].copy_from_slice(&src[..fh]);
        blit_x = x_pen;
        blit_y = top_y;
    }

    // 3. Light: AND each row with the checker mask, phase anchored at the
    //    cell bottom — EmuTOS blits bottom→top and rotates `lite_msk` per
    //    scanline (`vdi_tblit.S:1249`), so the bottom row uses the unrotated
    //    mask. Applied after outlining, matching `screen_blit`'s order.
    if style & T_LIGHT != 0 {
        let lite = ((f.lighten as u32) << 16) | f.lighten as u32; // 0x55555555
        for (b, row) in work.iter_mut().take(h).enumerate() {
            *row &= if (h - 1 - b) & 1 == 0 { lite } else { !lite };
        }
    }

    // 4. Effective cell size + pack rows to `ceil(cell_w/8)` bytes each.
    let skew_total = if skew { f.left_offset + f.right_offset } else { 0 };
    let mut cell_w = f.max_cell_width + skew_total;
    if outline {
        cell_w += OUTLINE_THICKNESS * 2;
    }
    let bpr = (cell_w as usize).div_ceil(8);
    let mut cell = [0u8; MAX_ROWS * 4];
    for (b, &row) in work.iter().take(h).enumerate() {
        for j in 0..bpr {
            cell[b * bpr + j] = (row >> (24 - 8 * j)) as u8;
        }
    }
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    backend.draw_glyph_sized(blit_x, blit_y, &cell[..h * bpr], cell_w, h as i16, vwk.text_color, mode, clip);
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

/// On-screen width of a `count`-char run in the current font and effects
/// (EmuTOS `calc_width`, `vdi_text.c:176-210`): the monospace advance plus
/// the once-per-string skew extent and the per-char outline pixels. Used
/// only for horizontal alignment; thicken adds nothing for a monospace
/// font, and scaled fonts (`vst_height`/`vst_point`) are out of scope here.
fn string_width(f: &SysFont, count: i16, style: i16) -> i16 {
    let mut width = count * f.max_cell_width;
    if style & T_SKEW != 0 {
        width += f.left_offset + f.right_offset;
    }
    if style & T_OUTLINE != 0 {
        width += count * OUTLINE_THICKNESS * 2;
    }
    width
}

/// Draw the underline run if the effect is set: a `ul_size`-thick line
/// below the baseline, spanning `[x0, x0+width)`. `y_cell_top` is the
/// rendered cell top (EmuTOS `DESTY`, i.e. already shifted up by outlining).
fn underline(backend: &dyn RasterBackend, f: &SysFont, x0: i16, y_cell_top: i16, width: i16, vwk: &Vwk, clip: ClipRect) {
    if vwk.style & T_UNDERLINE == 0 || width <= 0 {
        return;
    }
    // EmuTOS `vdi_text.c:399-413`: the underline row within the cell is
    // `top + underline`, where `underline = ul_size + (bottom > ul_size ? 1
    // : 0)`. The 8×16 font (bottom=2 > ul_size=1) drops the line to cell row
    // 15 (`13 + 2`); the 6×6/8×8 fonts (bottom == ul_size) sit it on the
    // descent line (`top + ul_size`), keeping it inside the smaller cell.
    let ul_off = f.ul_size + if f.bottom > f.ul_size { 1 } else { 0 };
    let y = y_cell_top + f.top + ul_off;
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    // Light dots the underline: `LN_MASK = lighten` (0x5555) when F_LIGHT is
    // set, else solid 0xFFFF (EmuTOS `vdi_text.c:513-516`). The backend
    // screen-aligns the mask for a horizontal line, giving a dotted rule.
    let mask = if vwk.style & T_LIGHT != 0 { f.lighten } else { 0xFFFF };
    for r in 0..f.ul_size {
        backend.draw_line(x0, y + r, x0 + width - 1, y + r, vwk.text_color, mode, clip, mask);
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
    let style = vwk.style;
    let (x, y) = (pb.ptsin(0), pb.ptsin(1));

    // Horizontal-alignment offset `delh`, including the outline start-1px and
    // the skew slant additives EmuTOS folds into it (`output_text`,
    // `vdi_text.c:352-390`). `x_pen0` is where char 0's glyph col 0 lands.
    let outline = if style & T_OUTLINE != 0 { OUTLINE_THICKNESS } else { 0 };
    let (d1, d2) = if style & T_SKEW != 0 {
        (f.left_offset, f.right_offset)
    } else {
        (0, 0)
    };
    let width = string_width(f, count, style);
    let mut delh = match vwk.h_align {
        1 => width / 2 - outline,  // centre
        2 => width - outline * 2,  // right
        _ => 0,                    // left (0)
    };
    delh += match vwk.v_align {
        0 => d1,                        // baseline
        1 => (f.half * d2) / f.top,     // half line
        2 => (f.ascent * d2) / f.top,   // ascent
        3 => 0,                         // bottom
        4 => (f.descent * d1) / f.bottom, // descent
        _ => d1 + d2,                   // top (5)
    };
    let x_pen0 = x - delh;
    let top_y = top_for(f, y, vwk.v_align);

    // Per-char advance: the monospace cell plus the outline's two pixels
    // (skew does not advance the pen — EmuTOS `upda_dst`, `vdi_textblit.c:1409`).
    let advance = f.max_cell_width + outline * 2;

    let clip = vwk.clip_rect();
    let mut cx = x_pen0;
    for i in 0..count as usize {
        draw_cell(backend, f, cx, top_y, pb.intin(i), vwk, clip);
        cx += advance;
    }
    underline(backend, f, x_pen0 - outline, top_y - outline, count * advance, vwk, clip);
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
        let code = pb.intin(2 + i);
        draw_cell(backend, f, cx, top_y, code, vwk, clip);
        cx += cell_w + char_extra;
        if code == b' ' as i16 {
            cx += word_extra;
        }
    }
    underline(backend, f, x0, top_y, cx - x0, vwk, clip);
}
