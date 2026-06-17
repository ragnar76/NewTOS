//! VDI output primitives — `vr_recfl` (filled rectangle) and `v_pline`
//! (polyline). Spec: `docs/api/vdi/output/`.
//!
//! The geometry/attribute handling lives here (hardware-independent);
//! the actual pixels go through the [`RasterBackend`]. Coordinates are
//! raster (RC, origin top-left), matching the screen workstation.

use super::backend::{ClipRect, PatternFill, RasterBackend};
use super::fill_patterns;
use super::vwk::Vwk;
use super::Vdipb;

const WM_REPLACE: i16 = 0;

/// The workstation's clip rectangle as an inclusive [`ClipRect`].
fn clip_of(vwk: &Vwk) -> ClipRect {
    ClipRect {
        xmn: vwk.xmn_clip,
        ymn: vwk.ymn_clip,
        xmx: vwk.xmx_clip,
        ymx: vwk.ymx_clip,
    }
}

/// Opcode 114 — `vr_recfl`: fill the rectangle `PTSIN[0..3]` with the
/// current fill style (interior + pattern/hatch + colour).
///
/// The active pattern comes from [`fill_patterns::select`] (`HOLLOW`
/// draws nothing); the writing mode chooses opaque (replace → clear bits
/// become background) vs. transparent. Wide-line perimeter (`vsf_perimeter`)
/// is handled by the GDP bar, not here.
pub fn vr_recfl(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let pattern = match fill_patterns::select(vwk) {
        Some(p) => p,
        None => return, // hollow → no fill
    };
    // Normalise the two corners (arb_corner) and clip to the workstation
    // rectangle (inclusive).
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
    let c = clip_of(vwk);
    let x1 = x1.max(c.xmn);
    let y1 = y1.max(c.ymn);
    let x2 = x2.min(c.xmx);
    let y2 = y2.min(c.ymx);
    if x1 > x2 || y1 > y2 {
        return; // fully clipped
    }
    let fill = PatternFill {
        fg: vwk.fill_color,
        bg: 0, // background = colour 0
        pattern,
        opaque: vwk.wrt_mode == WM_REPLACE,
    };
    backend.fill_pattern(x1, y1, x2 - x1 + 1, y2 - y1 + 1, fill);
}

/// Opcode 6 — `v_pline`: draw the polyline through `CONTRL[1]` vertices
/// in `PTSIN` with the current line colour.
///
/// Bring-up scope: one-pixel solid lines (width/dash/arrow styling lands
/// with the line-attribute milestone). Each segment is clipped to the
/// workstation rectangle by the backend.
pub fn v_pline(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let count = pb.contrl(1);
    if count < 2 {
        return;
    }
    let clip = clip_of(vwk);
    for i in 0..(count - 1) as usize {
        let x0 = pb.ptsin(2 * i);
        let y0 = pb.ptsin(2 * i + 1);
        let x1 = pb.ptsin(2 * i + 2);
        let y1 = pb.ptsin(2 * i + 3);
        backend.draw_line(x0, y0, x1, y1, vwk.line_color, clip);
    }
}
