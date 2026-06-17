//! VDI text output — `v_gtext` (opcode 8). Spec:
//! `docs/api/vdi/output/_text.md`.
//!
//! Bring-up scope: the single monospace 8×16 system [`font`], one glyph
//! per `INTIN` character code, with horizontal (left/centre/right) and
//! vertical (top/half/ascent/baseline/bottom) alignment. Effects
//! (bold/italic/underline) and rotation land later; the glyphs are blit
//! transparently in `text_color`.

use super::backend::RasterBackend;
use super::font;
use super::vwk::Vwk;
use super::Vdipb;

/// Opcode 8 — `v_gtext`: draw the `CONTRL[3]` characters in `INTIN`
/// (one code per WORD) at `PTSIN[0..1]`, in the current text colour and
/// alignment.
pub fn v_gtext(pb: &Vdipb, vwk: &Vwk, backend: &dyn RasterBackend) {
    let count = pb.contrl(3);
    if count <= 0 {
        return;
    }
    let x = pb.ptsin(0);
    let y = pb.ptsin(1);

    // Horizontal alignment shifts the run so the alignment point lands
    // at its left edge / centre / right edge.
    let width = count * font::CELL_W;
    let start_x = match vwk.h_align {
        1 => x - width / 2, // centre
        2 => x - width,     // right
        _ => x,             // left (0)
    };

    // Vertical alignment maps the reference line to the glyph top.
    let top_y = match vwk.v_align {
        0 => y - font::TOP,            // baseline
        1 => y - (font::TOP - font::HALF),   // half line
        2 => y - (font::TOP - font::ASCENT), // ascent (cap top)
        3 => y - (font::CELL_H - 1),  // bottom of cell
        4 => y - (font::TOP + font::DESCENT), // descent line
        _ => y,                       // top of cell (5)
    };

    let mut cx = start_x;
    for i in 0..count as usize {
        let code = pb.intin(i) as u16;
        backend.draw_glyph(cx, top_y, font::glyph(code), vwk.text_color);
        cx += font::CELL_W;
    }
}
