//! VDI generalized drawing primitives — `v_gdp` (opcode 11). Spec:
//! `docs/api/vdi/output/_gdp.md`. The sub-function is in `CONTRL[5]`.
//!
//! Every sub-primitive reduces to the line / fill primitives in
//! [`primitives`](super::primitives); `v_gdp` only generates geometry.

use super::backend::{PatternFill, RasterBackend, WriteMode};
use super::fill_patterns;
use super::primitives::{fill_polygon, line_mask, polyline, stroke};
use super::vwk::Vwk;
use super::Vdipb;

// GDP sub-function codes (`CONTRL[5]`).
const BAR: i16 = 1;
const ARC: i16 = 2;
const PIESLICE: i16 = 3;
const CIRCLE: i16 = 4;
const ELLIPSE: i16 = 5;
const ELLARC: i16 = 6;
const ELLPIE: i16 = 7;
const RBOX: i16 = 8;
const RFBOX: i16 = 9;
const JUSTIFIED: i16 = 10;

/// Opcode 11 — `v_gdp`: dispatch on `CONTRL[5]`.
pub fn v_gdp(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    match pb.contrl(5) {
        BAR => bar(pb, vwk, backend),
        CIRCLE | ELLIPSE => curve(pb, vwk, backend, true, false),
        PIESLICE | ELLPIE => curve(pb, vwk, backend, true, true),
        ARC | ELLARC => curve(pb, vwk, backend, false, false),
        RBOX => rbox(pb, vwk, backend, false),
        RFBOX => rbox(pb, vwk, backend, true),
        JUSTIFIED => super::text::v_justified(pb, vwk, backend),
        _ => {} // unknown sub-op → no-op (VDI has no error return)
    }
}

/// Normalised inclusive rectangle from `PTSIN[0..3]`.
fn rect(pb: &Vdipb) -> (i16, i16, i16, i16) {
    let (mut x1, mut y1) = (pb.ptsin(0), pb.ptsin(1));
    let (mut x2, mut y2) = (pb.ptsin(2), pb.ptsin(3));
    if x1 > x2 {
        core::mem::swap(&mut x1, &mut x2);
    }
    if y1 > y2 {
        core::mem::swap(&mut y1, &mut y2);
    }
    (x1, y1, x2, y2)
}

/// The current interior fill descriptor. `HOLLOW` is not a no-op: its
/// all-zero pattern erases the interior to pen 0 in Replace mode and
/// paints the fill colour in Reverse-transparent mode (EmuTOS
/// `st_fl_ptr` HOLLOW `patptr=&HOLLOW`, `swblit_rect_common16`).
fn fill_of(vwk: &Vwk) -> PatternFill<'_> {
    PatternFill {
        fg: vwk.fill_color,
        bg: 0,
        pattern: fill_patterns::select(vwk),
        mode: WriteMode::from_wrt_mode(vwk.wrt_mode),
    }
}

/// Sub-op 1 — `v_bar`: a filled rectangle plus, if `fill_per`, a solid
/// perimeter in the fill colour.
fn bar(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let (x1, y1, x2, y2) = rect(pb);
    let clip = vwk.clip_rect();
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    backend.fill_pattern(x1, y1, x2 - x1 + 1, y2 - y1 + 1, fill_of(vwk), clip);
    if vwk.fill_per {
        let pts = [(x1, y1), (x2, y1), (x2, y2), (x1, y2), (x1, y1)];
        polyline(backend, 5, |i| pts[i], vwk.fill_color, mode, clip, 0xFFFF);
    }
}

/// Sub-ops 2–7 — arc / pieslice / circle / ellipse / ellarc / ellpie.
/// `closed` fills (circle/ellipse/pie); open arcs are stroked. `to_centre`
/// closes the wedge to the centre (pieslice/ellpie).
fn curve(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend, closed: bool, to_centre: bool) {
    let sub = pb.contrl(5);
    let (cx, cy) = (pb.ptsin(0), pb.ptsin(1));
    // Radii: ellipse family carries xrad/yrad in PTSIN[2..3]; the round
    // family a single radius (circle PTSIN[4], arc/pie PTSIN[6]).
    let (xrad, yrad) = match sub {
        ELLIPSE | ELLARC | ELLPIE => (pb.ptsin(2).abs(), pb.ptsin(3).abs()),
        CIRCLE => {
            let r = pb.ptsin(4).abs();
            (r, r)
        }
        _ => {
            let r = pb.ptsin(6).abs();
            (r, r)
        }
    };
    if xrad == 0 && yrad == 0 {
        return;
    }
    // Angles in tenths of a degree (CCW). Full shapes force 0..3600.
    let full = matches!(sub, CIRCLE | ELLIPSE);
    let (begin, end) = if full { (0i32, 3600i32) } else { (pb.intin(0) as i32, pb.intin(1) as i32) };
    // EmuTOS `gdp_curve` (vdi_gdp.c:412-414): `del_ang = end - begin;
    // if (del_ang < 0) del_ang += TWOPI;`. Only a *negative* sweep wraps —
    // `begin == end` yields a zero sweep (a degenerate point), NOT a full
    // circle. Full shapes (circle/ellipse) are forced to 0..3600 above.
    let mut sweep = end - begin;
    if sweep < 0 {
        sweep += 3600;
    }
    // Flatten into segments (one per ~6°, min 16, max MAX_ARC).
    const MAX_ARC: usize = 128;
    let steps = ((sweep / 60).max(16) as usize).min(MAX_ARC);
    let mut pts = [(0i16, 0i16); MAX_ARC + 2];
    let mut n = 0;
    for s in 0..=steps {
        let ang = begin + sweep * s as i32 / steps as i32;
        let (sin, cos) = super::trig::sincos(ang);
        // x = cx + xrad*cos, y = cy - yrad*sin (screen y grows downward).
        let x = cx as i32 + (xrad as i32 * cos) / 32767;
        let y = cy as i32 - (yrad as i32 * sin) / 32767;
        pts[n] = (x as i16, y as i16);
        n += 1;
    }
    if to_centre {
        pts[n] = (cx, cy);
        n += 1;
    }
    let clip = vwk.clip_rect();
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    if closed || to_centre {
        fill_polygon(backend, n, &pts, fill_of(vwk), clip);
    }
    // Perimeter. Open arc/ellarc are stroked with the line attributes
    // (EmuTOS `clc_arc` → `set_LN_MASK(vwk); polyline(...)` in
    // `line_color`, vdi_gdp.c:212-213 — so the current line style dashes
    // the stroke). For a wide line EmuTOS uses `wideline` (always solid);
    // our `polyline`/`segment` matches that: width > 1 ignores the mask.
    // Closed circle/ellipse/pieslice/ellpie go through EmuTOS `polygon()`,
    // which draws the boundary — only when `fill_per` — in the **fill
    // colour**, solid and width 1 (`vdi_fill.c:606-608`).
    if !closed && !to_centre {
        // Open arc/ellarc: stroked with the line attributes (width 1 dashes
        // via line_mask; width > 1 uses the wide line).
        stroke(backend, &mut pts, n, vwk, vwk.line_color, mode, clip, line_mask(vwk));
    } else if vwk.fill_per {
        polyline(backend, n + 1, |i| pts[i % n], vwk.fill_color, mode, clip, 0xFFFF);
    }
}

/// Sub-ops 8/9 — rounded box. `filled` chooses `v_rfbox` vs `v_rbox`.
/// The corner radius is derived from the smaller rectangle dimension.
fn rbox(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend, filled: bool) {
    let (x1, y1, x2, y2) = rect(pb);
    let w = x2 - x1;
    let h = y2 - y1;
    // EmuTOS: corner ~ 1/8 of the shorter side, with a small floor.
    let r = (w.min(h) / 8).clamp(1, 16);
    // Build the rounded outline from four quarter-arcs (8 segments each).
    // One extra slot holds the explicit closing vertex for the wide-line
    // stroke (which detects a closed polyline via `pts[count-1] == pts[0]`).
    const SEG: usize = 8;
    let mut pts = [(0i16, 0i16); SEG * 4 + 5];
    let mut n = 0;
    // Corner centres: TR, TL, BL, BR (CCW), each sweeping 90°.
    let corners = [
        (x2 - r, y1 + r, 0i16),    // top-right, 0°..90°
        (x1 + r, y1 + r, 900i16),  // top-left, 90°..180°
        (x1 + r, y2 - r, 1800i16), // bottom-left, 180°..270°
        (x2 - r, y2 - r, 2700i16), // bottom-right, 270°..360°
    ];
    for &(ccx, ccy, base) in &corners {
        for s in 0..=SEG {
            let ang = base as i32 + 900 * s as i32 / SEG as i32;
            let (sin, cos) = super::trig::sincos(ang);
            let x = ccx as i32 + (r as i32 * cos) / 32767;
            let y = ccy as i32 - (r as i32 * sin) / 32767;
            if n < pts.len() {
                pts[n] = (x as i16, y as i16);
                n += 1;
            }
        }
    }
    let clip = vwk.clip_rect();
    let mode = WriteMode::from_wrt_mode(vwk.wrt_mode);
    if filled {
        fill_polygon(backend, n, &pts, fill_of(vwk), clip);
        if vwk.fill_per {
            polyline(backend, n + 1, |i| pts[i % n], vwk.fill_color, mode, clip, 0xFFFF);
        }
    } else {
        // Closed outline: append the closing vertex so a wide stroke sees a
        // closed polyline (round joints all around).
        pts[n] = pts[0];
        stroke(backend, &mut pts, n + 1, vwk, vwk.line_color, mode, clip, 0xFFFF);
    }
}
