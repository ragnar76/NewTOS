//! VDI attribute setters needed for the first drawing primitives.
//!
//! Spec: `docs/api/vdi/attributes/`. Each validates its input into the
//! [`Vwk`](super::vwk::Vwk) and echoes the accepted value; none can
//! fail (no VDI error return). Only the setters the bring-up demo uses
//! are here (line/fill colour + fill interior); the rest land with the
//! opcodes that consume them.

use super::vwk::{validate_color_index, Vwk};
use super::Vdipb;

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

/// Opcode 23 — `vsf_interior`: set the fill interior style
/// (0 = hollow, 1 = solid, 2 = pattern, 3 = hatch, 4 = user).
pub fn vsf_interior(pb: &Vdipb, vwk: &mut Vwk) {
    let s = pb.intin(0);
    vwk.fill_style = if (0..=4).contains(&s) { s } else { 0 };
    pb.set_intout(0, vwk.fill_style);
    pb.set_contrl(4, 1);
}
