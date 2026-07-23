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

/// One assembled glyph cell: `h` rows of a monochrome bitmap, `cell_w`
/// pixels wide (bit 31 of each `u32` = leftmost pixel). This is the buffer
/// EmuTOS' `pre_blit` produces — the glyph with thicken/skew/outline/light
/// already applied — ready to be blitted directly (`chup == 0`) or
/// transformed by [`rotate_cell`] before the blit (`chup != 0`).
#[derive(Clone, Copy)]
struct GlyphCell {
    /// Packed rows, bit 31 = leftmost pixel; `h` rows, `cell_w` bits wide.
    rows: [u32; MAX_ROWS],
    /// Effective cell width in pixels (glyph advance + skew + outline).
    cell_w: i16,
    /// Number of rows in the cell.
    h: i16,
}

/// Assemble one glyph into a [`GlyphCell`] with the active text effects
/// baked in — EmuTOS `pre_blit` (`vdi_textblit.c:352-436`): thicken + skew
/// build the source, `outline()` turns it into a hollow contour, and the
/// light mask dims it. Position-independent: the caller places the result
/// (directly for `chup == 0`, or after [`rotate_cell`] for a right angle).
fn build_cell(f: &SysFont, code: i16, vwk: &Vwk) -> GlyphCell {
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
    let h;
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
    } else {
        h = fh;
        work[..fh].copy_from_slice(&src[..fh]);
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

    // 4. Effective cell width (glyph advance + skew extent + outline pixels).
    let skew_total = if skew { f.left_offset + f.right_offset } else { 0 };
    let mut cell_w = f.max_cell_width + skew_total;
    if outline {
        cell_w += OUTLINE_THICKNESS * 2;
    }
    GlyphCell { rows: work, cell_w, h: h as i16 }
}

/// Pack a [`GlyphCell`] to `ceil(cell_w/8)` bytes per row and blit it with
/// its top-left at `(x, y)` in the workstation writing mode + text colour.
fn blit_cell(backend: &dyn RasterBackend, cell: &GlyphCell, x: i16, y: i16, vwk: &Vwk, clip: ClipRect) {
    let cell_w = cell.cell_w;
    let h = cell.h as usize;
    let bpr = (cell_w as usize).div_ceil(8);
    let mut bytes = [0u8; MAX_ROWS * 4];
    for (b, &row) in cell.rows.iter().take(h).enumerate() {
        for j in 0..bpr {
            bytes[b * bpr + j] = (row >> (24 - 8 * j)) as u8;
        }
    }
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    backend.draw_glyph_sized(x, y, &bytes[..h * bpr], cell_w, cell.h, vwk.text_color, mode, clip);
}

/// Blit one glyph horizontally (`chup == 0`): glyph col 0 at `x_pen`, cell
/// top at `top_y` (both before the outline shift). The horizontal
/// `v_gtext`/`v_justified` path; byte-identical to the pre-rotation code.
fn draw_cell(backend: &dyn RasterBackend, f: &SysFont, x_pen: i16, top_y: i16, code: i16, vwk: &Vwk, clip: ClipRect) {
    // Outlined text starts 1 px earlier and 1 row higher (EmuTOS `outline`
    // insets the glyph; `output_text` subtracts `outline` from DESTX/DESTY,
    // `vdi_text.c:410-411`).
    let out_shift = if vwk.style & T_OUTLINE != 0 { OUTLINE_THICKNESS } else { 0 };
    let cell = build_cell(f, code, vwk);
    blit_cell(backend, &cell, x_pen - out_shift, top_y - out_shift, vwk, clip);
}

/// Rotate an assembled [`GlyphCell`] by a right angle — EmuTOS `rotate`
/// (`vdi_textblit.c:442-554`). `chup` is 900 (90° CCW), 1800 (180°) or 2700
/// (90° CW). 90°/270° swap the cell's width and height; 180° preserves them.
/// The source pixel `(sx, sy)` in a `cell_w × h` cell maps to:
///
/// * 900:  `(sy, cell_w-1-sx)` — top-left → bottom-left (CCW);
/// * 1800: `(cell_w-1-sx, h-1-sy)` — top-left → bottom-right;
/// * 2700: `(h-1-sy, sx)` — top-left → top-right (CW).
///
/// These reproduce the bit transforms EmuTOS' `rotate` performs (the 180°
/// nybble-reverse-and-stack, and the 90°/270° transpose loops).
fn rotate_cell(cell: &GlyphCell, chup: i16) -> GlyphCell {
    let w = cell.cell_w as usize;
    let h = cell.h as usize;
    let mut out = GlyphCell { rows: [0u32; MAX_ROWS], cell_w: 0, h: 0 };
    for sy in 0..h {
        for sx in 0..w {
            if cell.rows[sy] & (0x8000_0000u32 >> sx) == 0 {
                continue;
            }
            let (dx, dy) = match chup {
                900 => (sy, w - 1 - sx),
                1800 => (w - 1 - sx, h - 1 - sy),
                _ => (h - 1 - sy, sx), // 2700
            };
            out.rows[dy] |= 0x8000_0000u32 >> dx;
        }
    }
    // 90°/270° swap the dimensions; 180° keeps them.
    if chup == 1800 {
        out.cell_w = cell.cell_w;
        out.h = cell.h;
    } else {
        out.cell_w = cell.h;
        out.h = cell.cell_w;
    }
    out
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
/// alignment, effects and selected system font. A right-angle `chup`
/// (900/1800/2700) rotates the run (EmuTOS `output_text` + `rotate`); any
/// other angle — 0, or the 3600 `vst_rotation` can round up to — renders
/// horizontally (EmuTOS' `switch(chup)` `default` case).
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
    // `vdi_text.c:352-390`). Computed the same for every `chup` — EmuTOS runs
    // both alignment switches before the rotation switch.
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

    let clip = vwk.clip_rect();

    // Right-angle rotation takes the assemble→rotate→blit path; every other
    // angle (0 and 3600) falls through to the horizontal path — matching
    // EmuTOS' `switch(vwk->chup)` where only 900/1800/2700 are cased.
    if matches!(vwk.chup, 900 | 1800 | 2700) {
        output_text_rotated(pb, vwk, backend, f, count, x, y, delh, outline, clip);
        return;
    }

    // Horizontal (`chup == 0`) — unchanged. `x_pen0` is where char 0's glyph
    // col 0 lands.
    let x_pen0 = x - delh;
    let top_y = top_for(f, y, vwk.v_align);

    // Per-char advance: the monospace cell plus the outline's two pixels
    // (skew does not advance the pen — EmuTOS `upda_dst`, `vdi_textblit.c:1409`).
    let advance = f.max_cell_width + outline * 2;

    let mut cx = x_pen0;
    for i in 0..count as usize {
        draw_cell(backend, f, cx, top_y, pb.intin(i), vwk, clip);
        cx += advance;
    }
    underline(backend, f, x_pen0 - outline, top_y - outline, count * advance, vwk, clip);
}

/// Draw a `v_gtext` run rotated to a right angle — EmuTOS `output_text`'s
/// `chup` 900/1800/2700 cases (`vdi_text.c:408-543`) driving `text_blt` +
/// `rotate` (`vdi_textblit.c:442-554`, `1331-1342`, `1419-1432`). Each glyph
/// is assembled ([`build_cell`]), rotated ([`rotate_cell`]) and blitted; the
/// pen advances along the rotated baseline and the underline (if any) runs
/// parallel to it. `delh`/`outline` are the shared values from `v_gtext`.
#[allow(clippy::too_many_arguments)]
fn output_text_rotated(
    pb: &Vdipb,
    vwk: &Vwk,
    backend: &dyn RasterBackend,
    f: &SysFont,
    count: i16,
    x: i16,
    y: i16,
    delh: i16,
    outline: i16,
    clip: ClipRect,
) {
    let chup = vwk.chup;
    let top = f.top;
    let bottom = f.bottom;

    // Underline row offset within the cell (EmuTOS `output_text`,
    // `vdi_text.c:399-405`). The scaled `-1` doubling case belongs to the
    // deferred DDA render scaling, so `scaled` is false here.
    let underline_off = f.ul_size + if f.bottom > f.ul_size { 1 } else { 0 };

    // Vertical-alignment shift `delv` (EmuTOS `switch(vwk->v_align)` `delv`,
    // `vdi_text.c:366-390`).
    let delv = match vwk.v_align {
        0 => f.top,
        1 => f.top - f.half,
        2 => f.top - f.ascent,
        3 => f.top + f.bottom,
        4 => f.top + f.descent,
        _ => 0,
    };

    // The per-glyph cell width (incl. skew + outline) shifts a glyph's
    // top-left in `text_blt`'s `chup` switch; the pen advance excludes skew
    // (EmuTOS `upda_dst` recomputes `delx` without `LOFF+ROFF`,
    // `vdi_textblit.c:1403-1417`). Monospace → both are constant per run.
    let skew_total = if vwk.style & T_SKEW != 0 { f.left_offset + f.right_offset } else { 0 };
    let out2 = outline * 2;
    let cell_adj = f.max_cell_width + skew_total + out2;
    let advance = f.max_cell_width + out2;

    // Anchor (first glyph), per-glyph blit adjustment, pen advance and
    // underline params, mirroring EmuTOS `output_text` (`vdi_text.c:417-440`)
    // and `text_blt`'s `chup` switch (`vdi_textblit.c:1331-1342`). `(top+bottom)`
    // is `form_height - 1`.
    let ax;
    let ay;
    let (adj_x, adj_y); // vars.DESTX/Y adjustment applied at each glyph
    let (adv_x, adv_y); // global DESTX/Y advance per glyph
    let startx;
    let starty;
    let (xfact, yfact); // underline thickness step
    let vertical; // underline orientation
    match chup {
        900 => {
            ax = x - delv - outline;
            ay = y + delh + outline + 1;
            adj_x = 0;
            adj_y = -cell_adj; // vars.DESTY -= delx
            adv_x = 0;
            adv_y = -advance; // DESTY -= delx (move up)
            startx = ax + top + underline_off;
            starty = ay;
            xfact = 1;
            yfact = 0;
            vertical = true;
        }
        1800 => {
            ax = x + delh + outline + 1;
            ay = y - ((top + bottom) - delv) - outline;
            adj_x = -cell_adj; // vars.DESTX -= delx
            adj_y = 0;
            adv_x = -advance; // DESTX -= delx (move left)
            adv_y = 0;
            startx = ax;
            starty = ay + bottom - underline_off;
            xfact = 0;
            yfact = -1;
            vertical = false;
        }
        _ => {
            // 2700 — no vars.DEST adjustment in `text_blt`'s switch.
            ax = x - ((top + bottom) - delv) - outline;
            ay = y - delh - outline;
            adj_x = 0;
            adj_y = 0;
            adv_x = 0;
            adv_y = advance; // DESTY += delx (move down)
            startx = ax + bottom - underline_off;
            starty = ay;
            xfact = -1;
            yfact = 0;
            vertical = true;
        }
    }

    let mut gx = ax;
    let mut gy = ay;
    for i in 0..count as usize {
        let cell = build_cell(f, pb.intin(i), vwk);
        let rot = rotate_cell(&cell, chup);
        blit_cell(backend, &rot, gx + adj_x, gy + adj_y, vwk, clip);
        gx += adv_x;
        gy += adv_y;
    }

    // Underline parallel to the rotated baseline (EmuTOS `output_text`,
    // `vdi_text.c:500-543`): vertical for 900/2700, horizontal for 1800; the
    // far endpoint is the pen's final position (`gx`/`gy`). `ul_size` thin
    // lines stepped by (xfact, yfact) give the thickness.
    if vwk.style & T_UNDERLINE != 0 {
        let mask = if vwk.style & T_LIGHT != 0 { f.lighten } else { 0xFFFF };
        let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
        let mut x1 = startx;
        let mut y1 = starty;
        let (mut x2, mut y2) = if vertical { (startx, gy) } else { (gx, starty) };
        for _ in 0..f.ul_size {
            backend.draw_line(x1, y1, x2, y2, vwk.text_color, mode, clip, mask);
            x1 += xfact;
            x2 += xfact;
            y1 += yfact;
            y2 += yfact;
        }
    }
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
