//! VDI inquiry opcodes — read-only queries of the workstation attribute
//! state, the palette, a pixel, and the keyboard. Spec:
//! `docs/api/vdi/inquiry/`.
//!
//! Colours round-trip through `MAP_COL`/`REV_MAP_COL`; NewTOS stores PEL
//! directly in the `Vwk` and uses an identity 16-entry map, so the VDI
//! index and the PEL coincide.

use super::backend::RasterBackend;
use super::font;
use super::req_col;
use super::vwk::Vwk;
use super::Vdipb;

// Text effect bits queried by vqt_fontinfo (EmuTOS `F_THICKEN`/`F_SKEW`).
const F_THICKEN: i16 = 0x01;
const F_SKEW: i16 = 0x04;

/// Opcode 35 — `vql_attributes`: read back the line attributes.
pub fn vql_attributes(pb: &Vdipb, vwk: &Vwk) {
    pb.set_intout(0, vwk.line_index + 1);
    pb.set_intout(1, vwk.line_color as i16);
    pb.set_intout(2, vwk.wrt_mode + 1);
    pb.set_ptsout(0, vwk.line_width);
    pb.set_ptsout(1, 0);
    pb.set_contrl(4, 3);
    pb.set_contrl(2, 1);
}

/// Opcode 36 — `vqm_attributes`: read back the marker attributes. Note
/// `mark_index` is returned **0-based** (an EmuTOS/TOS ABI quirk).
pub fn vqm_attributes(pb: &Vdipb, vwk: &Vwk) {
    pb.set_intout(0, vwk.mark_index);
    pb.set_intout(1, vwk.mark_color as i16);
    pb.set_intout(2, vwk.wrt_mode + 1);
    pb.set_ptsout(0, 0);
    pb.set_ptsout(1, vwk.mark_height);
    pb.set_contrl(4, 3);
    pb.set_contrl(2, 1);
}

/// Opcode 37 — `vqf_attributes`: read back the fill attributes.
pub fn vqf_attributes(pb: &Vdipb, vwk: &Vwk) {
    pb.set_intout(0, vwk.fill_style);
    pb.set_intout(1, vwk.fill_color as i16);
    pb.set_intout(2, vwk.fill_index + 1);
    pb.set_intout(3, vwk.wrt_mode + 1);
    pb.set_intout(4, if vwk.fill_per { 1 } else { 0 });
    pb.set_contrl(4, 5);
}

/// Opcode 38 — `vqt_attributes`: read back the text attributes + the
/// current font's cell metrics.
pub fn vqt_attributes(pb: &Vdipb, vwk: &Vwk) {
    pb.set_intout(0, font::FONT_ID);
    pb.set_intout(1, vwk.text_color as i16);
    pb.set_intout(2, vwk.chup);
    pb.set_intout(3, vwk.h_align);
    pb.set_intout(4, vwk.v_align);
    pb.set_intout(5, vwk.wrt_mode + 1);
    pb.set_ptsout(0, font::MAX_CHAR_WIDTH);
    pb.set_ptsout(1, font::TOP);
    pb.set_ptsout(2, font::CELL_W);
    pb.set_ptsout(3, font::TOP + font::BOTTOM + 1);
    pb.set_contrl(4, 6);
    pb.set_contrl(2, 2);
}

/// Opcode 26 — `vq_color`: read a palette entry. `INTIN[0]` = index,
/// `INTIN[1]` = flag (0 = last *requested* value, ≠0 = *actual*). EmuTOS
/// `vdi_col.c:843-936`: flag 0 returns the raw `REQ_COL` request (which
/// may lie outside 0..1000), flag ≠0 the hardware DAC read-back. NewTOS
/// keeps the raw request in the `REQ_COL` shadow and reads the clamped/
/// quantised palette from the backend for the "actual" path.
pub fn vq_color(pb: &Vdipb, ncol: u16, backend: &dyn RasterBackend) {
    let idx = pb.intin(0);
    let flag = pb.intin(1);
    pb.set_intout(1, 0);
    pb.set_intout(2, 0);
    pb.set_intout(3, 0);
    if idx < 0 || idx as u16 >= ncol {
        pb.set_intout(0, -1);
        pb.set_contrl(4, 4);
        return;
    }
    pb.set_intout(0, idx);
    // flag 0: the raw last-requested value (REQ_COL); registers never set
    // by vs_color fall back to the live palette (the hardware default).
    // flag ≠0: the actual hardware palette read-back.
    let raw = if flag == 0 {
        // SAFETY: single trap path (see `req_col`).
        unsafe { req_col::requested(idx as u16) }
    } else {
        None
    };
    let (r, g, b) = match raw {
        Some(rgb) => rgb,
        None => {
            let (r, g, b) = backend.query_color(idx as u16);
            (r as i16, g as i16, b as i16)
        }
    };
    pb.set_intout(1, r);
    pb.set_intout(2, g);
    pb.set_intout(3, b);
    pb.set_contrl(4, 4);
}

/// Opcode 105 — `v_get_pixel`: read the pixel at `PTSIN[0..1]`.
/// `INTOUT[0]` = pixel value (= PEL on the palette screen),
/// `INTOUT[1]` = VDI colour index (`REV_MAP_COL`, identity here).
pub fn v_get_pixel(pb: &Vdipb, backend: &dyn RasterBackend) {
    let pel = backend.read_pixel(pb.ptsin(0), pb.ptsin(1));
    pb.set_intout(0, pel as i16);
    pb.set_intout(1, pel as i16);
    pb.set_contrl(4, 2);
}

/// Opcode 128 — `vq_key_s`: the current keyboard shift-state bitmask
/// (`MODE_*`), from the IKBD decoder via the host.
pub fn vq_key_s(pb: &Vdipb) {
    let state = crate::host::host().map(|h| h.keyboard_shift_state()).unwrap_or(0);
    pb.set_intout(0, state as i16);
    pb.set_contrl(4, 1);
}

/// Opcode 116 — `vqt_extent`: the bounding box of the `CONTRL[3]`-char
/// string in `INTIN`, as four corner points in `PTSOUT[0..7]`, rotated
/// per `chup` (90° snaps).
pub fn vqt_extent(pb: &Vdipb, vwk: &Vwk) {
    let count = pb.contrl(3).max(0);
    let width = count * font::CELL_W;
    let height = font::TOP + font::BOTTOM + 1; // cell height
    for i in 0..8 {
        pb.set_ptsout(i, 0);
    }
    match vwk.chup {
        900 => {
            pb.set_ptsout(0, height);
            pb.set_ptsout(2, height);
            pb.set_ptsout(3, width);
            pb.set_ptsout(5, width);
        }
        1800 => {
            pb.set_ptsout(0, width);
            pb.set_ptsout(1, height);
            pb.set_ptsout(3, height);
            pb.set_ptsout(6, width);
        }
        2700 => {
            pb.set_ptsout(1, width);
            pb.set_ptsout(4, height);
            pb.set_ptsout(6, height);
            pb.set_ptsout(7, width);
        }
        _ => {
            // 0 / 3600: P0=(0,0) P1=(w,0) P2=(w,h) P3=(0,h)
            pb.set_ptsout(2, width);
            pb.set_ptsout(4, width);
            pb.set_ptsout(5, height);
            pb.set_ptsout(7, height);
        }
    }
    pb.set_contrl(2, 4);
}

/// Opcode 117 — `vqt_width`: the cell + horizontal offsets of char
/// `INTIN[0]`. `INTOUT[0]` = char (or −1 if outside the font range);
/// `PTSOUT[0]` = cell width, `PTSOUT[2]`/`[4]` = left/right offsets (0
/// for the monospace system font).
pub fn vqt_width(pb: &Vdipb, _vwk: &Vwk) {
    let k = pb.intin(0);
    pb.set_ptsout(2, 0);
    pb.set_ptsout(4, 0);
    pb.set_ptsout(1, 0);
    pb.set_ptsout(3, 0);
    pb.set_ptsout(5, 0);
    if !(font::FIRST_ADE..=font::LAST_ADE).contains(&k) {
        pb.set_intout(0, -1);
    } else {
        pb.set_intout(0, k);
        pb.set_ptsout(0, font::CELL_W); // monospace advance
    }
    pb.set_contrl(4, 1);
    pb.set_contrl(2, 3);
}

/// Opcode 130 — `vqt_name`: the `INTIN[0]`-th (1-based) font face. Only
/// the system font exists, so every index returns it: `INTOUT[0]` = font
/// id, `INTOUT[1..]` = the name, one char per WORD, padded to `NAME_LEN`.
pub fn vqt_name(pb: &Vdipb) {
    pb.set_intout(0, font::FONT_ID);
    for i in 0..font::NAME_LEN {
        let ch = font::NAME.get(i).copied().unwrap_or(0);
        pb.set_intout(1 + i, ch as i16);
    }
    pb.set_contrl(4, 1 + font::NAME_LEN as i16);
}

/// Opcode 131 — `vqt_fontinfo`: the current font's metrics, with the
/// thicken/skew slots reflecting the active text `style`.
pub fn vqt_fontinfo(pb: &Vdipb, vwk: &Vwk) {
    pb.set_intout(0, font::FIRST_ADE);
    pb.set_intout(1, font::LAST_ADE);
    pb.set_ptsout(0, font::CELL_W); // max_cell_width
    pb.set_ptsout(1, font::BOTTOM);
    pb.set_ptsout(2, if vwk.style & F_THICKEN != 0 { font::THICKEN } else { 0 });
    pb.set_ptsout(3, font::DESCENT);
    if vwk.style & F_SKEW != 0 {
        pb.set_ptsout(4, font::LEFT_OFFSET);
        pb.set_ptsout(5, font::HALF);
        pb.set_ptsout(6, font::RIGHT_OFFSET);
    } else {
        pb.set_ptsout(4, 0);
        pb.set_ptsout(5, font::HALF);
        pb.set_ptsout(6, 0);
    }
    pb.set_ptsout(7, font::ASCENT);
    pb.set_ptsout(8, 0);
    pb.set_ptsout(9, font::TOP);
    pb.set_contrl(4, 2);
    pb.set_contrl(2, 5);
}
