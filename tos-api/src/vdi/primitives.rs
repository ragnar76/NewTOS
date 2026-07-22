//! VDI output primitives — `vr_recfl` (filled rectangle) and `v_pline`
//! (polyline). Spec: `docs/api/vdi/output/`.
//!
//! The geometry/attribute handling lives here (hardware-independent);
//! the actual pixels go through the [`RasterBackend`]. Coordinates are
//! raster (RC, origin top-left), matching the screen workstation.

use super::backend::{ClipRect, PatternFill, Pel, RasterBackend, WriteMode};
use super::fill_patterns;
use super::vwk::Vwk;
use super::Vdipb;

/// The six built-in line-style dash masks (`vdi_line.c LINE_STYLE`);
/// index 6 (style 7) is the user mask `ud_ls`.
const LINE_STYLE: [u16; 6] = [0xFFFF, 0xFFF0, 0xC0C0, 0xFF18, 0xFF00, 0xF191];

/// The active line dash mask for the workstation's line style.
pub(super) fn line_mask(vwk: &Vwk) -> u16 {
    let idx = vwk.line_index;
    if (0..6).contains(&idx) {
        LINE_STYLE[idx as usize]
    } else {
        vwk.ud_ls // style 7 → user-defined
    }
}

/// Draw a single segment honouring the line width (odd) and dash mask.
/// A width &gt; 1 draws parallel offset copies perpendicular to the
/// dominant axis (solid), matching the VDI `wideline` thick-line look.
#[allow(clippy::too_many_arguments)]
fn segment(
    backend: &dyn RasterBackend,
    x0: i16,
    y0: i16,
    x1: i16,
    y1: i16,
    color: Pel,
    mode: WriteMode,
    clip: ClipRect,
    width: i16,
    mask: u16,
) {
    if width <= 1 {
        backend.draw_line(x0, y0, x1, y1, color, mode, clip, mask);
        return;
    }
    // A wide line draws parallel solid copies (`wideline`, always solid).
    let half = (width - 1) / 2;
    let horizontalish = (x1 as i32 - x0 as i32).abs() >= (y1 as i32 - y0 as i32).abs();
    for off in -half..=half {
        if horizontalish {
            backend.draw_line(x0, y0 + off, x1, y1 + off, color, mode, clip, 0xFFFF);
        } else {
            backend.draw_line(x0 + off, y0, x1 + off, y1, color, mode, clip, 0xFFFF);
        }
    }
}

/// Draw a polyline through `n` `(x,y)` pairs read from `pts(i)` in the
/// given writing `mode` (EmuTOS `polyline` → `abline(&line, vwk->wrt_mode,
/// color)` — perimeters and strokes honour the workstation mode).
#[allow(clippy::too_many_arguments)]
pub(super) fn polyline(
    backend: &dyn RasterBackend,
    n: usize,
    mut pt: impl FnMut(usize) -> (i16, i16),
    color: Pel,
    mode: WriteMode,
    clip: ClipRect,
    width: i16,
    mask: u16,
) {
    if n < 2 {
        return;
    }
    for i in 0..n - 1 {
        let (x0, y0) = pt(i);
        let (x1, y1) = pt(i + 1);
        segment(backend, x0, y0, x1, y1, color, mode, clip, width, mask);
    }
}

/// Opcode 114 — `vr_recfl`: fill the rectangle `PTSIN[0..3]` with the
/// current fill style (interior + pattern/hatch + colour).
///
/// The active pattern comes from [`fill_patterns::select`] (`HOLLOW`
/// draws nothing); the writing mode chooses opaque (replace → clear bits
/// become background) vs. transparent. The backend clips to the
/// workstation rectangle. Wide-line perimeter (`vsf_perimeter`) is
/// handled by the GDP bar, not here.
pub fn vr_recfl(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let pattern = match fill_patterns::select(vwk) {
        Some(p) => p,
        None => return, // hollow → no fill
    };
    // Normalise the two corners (arb_corner); the backend clips.
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
    let fill = PatternFill {
        fg: vwk.fill_color,
        bg: 0, // background = colour 0
        pattern,
        mode: WriteMode::from_wrt_mode(vwk.wrt_mode),
    };
    backend.fill_pattern(x1, y1, x2 - x1 + 1, y2 - y1 + 1, fill, vwk.clip_rect());
}

/// Maximum polygon vertices / scanline edge crossings handled by the
/// fixed-size fill buffers. This is EmuTOS' `MAX_VERTICES` (`config.h`,
/// make-512 profile = 512); it is also advertised as `INQ_TAB[14]` (see
/// [`super::workstation`], which references this same constant).
pub(super) const MAX_VERTS: usize = 512;

/// Scan-convert and fill the closed polygon `v[0..n]` with `fill`,
/// clipped to `clip`. Even-odd rule; each scanline span is a 1-pixel-tall
/// patterned rectangle, so the pattern stays screen-aligned.
pub(super) fn fill_polygon(
    backend: &dyn RasterBackend,
    n: usize,
    v: &[(i16, i16)],
    fill: PatternFill,
    clip: ClipRect,
) {
    if n < 3 {
        return;
    }
    let n = n.min(MAX_VERTS);
    let (mut ymin, mut ymax) = (i16::MAX, i16::MIN);
    for &(_, y) in &v[..n] {
        ymin = ymin.min(y);
        ymax = ymax.max(y);
    }
    ymin = ymin.max(clip.ymn);
    ymax = ymax.min(clip.ymx);
    let mut xs = [0i16; MAX_VERTS];
    for y in ymin..=ymax {
        // Collect edge crossings at scanline centre `y`.
        let mut cnt = 0usize;
        for i in 0..n {
            let (x0, y0) = v[i];
            let (x1, y1) = v[(i + 1) % n];
            let (ya, yb) = (y0 as i32, y1 as i32);
            let yc = y as i32;
            // Half-open rule avoids double-counting shared vertices.
            if (ya <= yc && yc < yb) || (yb <= yc && yc < ya) {
                let x = x0 as i32 + (yc - ya) * (x1 as i32 - x0 as i32) / (yb - ya);
                if cnt < MAX_VERTS {
                    xs[cnt] = x as i16;
                    cnt += 1;
                }
            }
        }
        // Sort the crossings (small insertion sort).
        for i in 1..cnt {
            let key = xs[i];
            let mut j = i;
            while j > 0 && xs[j - 1] > key {
                xs[j] = xs[j - 1];
                j -= 1;
            }
            xs[j] = key;
        }
        // Fill between consecutive pairs.
        let mut i = 0;
        while i + 1 < cnt {
            let xa = xs[i];
            let xb = xs[i + 1];
            if xb >= xa {
                backend.fill_pattern(xa, y, xb - xa + 1, 1, fill, clip);
            }
            i += 2;
        }
    }
}

/// Opcode 9 — `v_fillarea`: fill the closed polygon `PTSIN[0..2*n]` with
/// the current interior style. If `fill_per`, outline the perimeter.
pub fn v_fillarea(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let n = pb.contrl(1).max(0) as usize;
    if n < 3 {
        return;
    }
    let n = n.min(MAX_VERTS);
    let mut v = [(0i16, 0i16); MAX_VERTS];
    for (i, slot) in v[..n].iter_mut().enumerate() {
        *slot = (pb.ptsin(2 * i), pb.ptsin(2 * i + 1));
    }
    let clip = vwk.clip_rect();
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    if let Some(pattern) = fill_patterns::select(vwk) {
        let fill = PatternFill {
            fg: vwk.fill_color,
            bg: 0,
            pattern,
            mode,
        };
        fill_polygon(backend, n, &v, fill, clip);
    }
    if vwk.fill_per {
        // Outline: the polygon closed back to the first vertex, in the
        // fill colour and the workstation writing mode (EmuTOS `polygon`
        // → `polyline(vwk, ..., vwk->fill_color)`, `vdi_fill.c:608`).
        polyline(
            backend,
            n + 1,
            |i| v[i % n],
            vwk.fill_color,
            mode,
            clip,
            1,
            0xFFFF,
        );
    }
}

/// Opcode 6 — `v_pline`: draw the polyline through `CONTRL[1]` vertices
/// in `PTSIN` with the current line colour, width (odd) and dash style.
/// Each segment is clipped to the workstation rectangle by the backend.
pub fn v_pline(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let count = pb.contrl(1);
    if count < 2 {
        return;
    }
    polyline(
        backend,
        count as usize,
        |i| (pb.ptsin(2 * i), pb.ptsin(2 * i + 1)),
        vwk.line_color,
        WriteMode::from_wrt_mode(vwk.wrt_mode),
        vwk.clip_rect(),
        vwk.line_width,
        line_mask(vwk),
    );
}

/// The six marker glyphs as polyline vector tables (`vdi_marker.c`):
/// `[num_polylines, (count, dx,dy …) …]`, scaled by `mark_scale`.
const M_DOT: &[i16] = &[1, 2, 0, 0, 0, 0];
const M_PLUS: &[i16] = &[2, 2, 0, -3, 0, 3, 2, -4, 0, 4, 0];
const M_STAR: &[i16] = &[3, 2, 0, -3, 0, 3, 2, 3, 2, -3, -2, 2, 3, -2, -3, 2];
const M_SQUARE: &[i16] = &[1, 5, -4, -3, 4, -3, 4, 3, -4, 3, -4, -3];
const M_CROSS: &[i16] = &[2, 2, -4, -3, 4, 3, 2, -4, 3, 4, -3];
const M_DMND: &[i16] = &[1, 5, -4, 0, 0, -3, 4, 0, 0, 3, -4, 0];
const MARKERS: [&[i16]; 6] = [M_DOT, M_PLUS, M_STAR, M_SQUARE, M_CROSS, M_DMND];

/// Opcode 7 — `v_pmarker`: draw a marker glyph at each of the `CONTRL[1]`
/// points in `PTSIN`. Markers use solid, width-1 lines in `mark_color`,
/// scaled by `mark_scale`; clipping is always on.
pub fn v_pmarker(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let count = pb.contrl(1);
    if count < 1 {
        return;
    }
    let glyph = MARKERS[(vwk.mark_index.clamp(0, 5)) as usize];
    let scale = vwk.mark_scale.max(1);
    let clip = vwk.clip_rect();
    // Markers honour the workstation writing mode (EmuTOS `v_pmarker`
    // borrows the polyline path, which draws with `vwk->wrt_mode`).
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    for p in 0..count as usize {
        let (cx, cy) = (pb.ptsin(2 * p), pb.ptsin(2 * p + 1));
        let mut k = 1usize; // skip num_polylines header
        let num_lines = glyph[0];
        for _ in 0..num_lines {
            let npts = glyph[k] as usize;
            k += 1;
            polyline(
                backend,
                npts,
                |i| (cx + scale * glyph[k + 2 * i], cy + scale * glyph[k + 2 * i + 1]),
                vwk.mark_color,
                mode,
                clip,
                1,
                0xFFFF,
            );
            k += 2 * npts;
        }
    }
}
