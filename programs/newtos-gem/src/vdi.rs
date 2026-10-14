//! VDI trap binding (`svc #2`, selector 115).
//!
//! DESK43 U6b: the trap half of the kernel desktop's `kernel/src/desk/vdibind.rs`
//! (its line pen, user line style, polyline, clip and text-height queries)
//! merged with the calls the U6a runtime had here (a virtual workstation, fill,
//! text). The desktop keeps only what is desktop logic — its `gsx_*` emulation
//! of the AES's internal drawing helpers — on top of these bindings.
//!
//! ABI (`docs/sdk/05-abi-pitfalls.md` §A1; `sdk/gemlib/overlay/_gc_vdi.c`):
//! `x0` = 115, `x1` = the VDI parameter block — five consecutive native
//! pointers `contrl`, `intin`, `ptsin`, `intout`, `ptsout`; the dispatcher
//! picks the function from `contrl[0]` and returns `flip_y` in `x0`, which no
//! caller here needs.
//!
//! **Opcodes** carry EmuTOS's names where it has one (`aes/funcdef.h`, the
//! AES's own VDI binding — `POLYLINE`, `SET_WRITING_MODE`, …); the three calls
//! the AES never makes have none there and carry a name of the same form
//! (marked). Argument layouts: `docs/api/vdi/` (EmuTOS `vdi/vdi_main.c` jump
//! table; the NewTOS dispatch table `tos-api/src/vdi/mod.rs`). Function
//! signatures follow the classic GEM VDI bindings (`vs_clip(handle,
//! clip_flag, pxyarray)`, …).

use crate::aes::Grect;

/// `v_pline` (`aes/funcdef.h:24`).
pub const POLYLINE: i16 = 6;
/// `v_gtext` (`aes/funcdef.h:26`).
pub const TEXT: i16 = 8;
/// `vst_height` (`aes/funcdef.h:28`).
pub const SET_CHAR_HEIGHT: i16 = 12;
/// `vsl_color` (`aes/funcdef.h:32`).
pub const SET_LINE_COLOR: i16 = 17;
/// `vst_color` (`aes/funcdef.h:34`).
pub const SET_TEXT_COLOR: i16 = 22;
/// `vsf_interior` (`aes/funcdef.h:35`).
pub const SET_FILL_INTERIOR: i16 = 23;
/// `vsf_color` (`aes/funcdef.h:37`).
pub const SET_FILL_COLOR: i16 = 25;
/// `vswr_mode` (`aes/funcdef.h:42`).
pub const SET_WRITING_MODE: i16 = 32;
/// `vqt_attributes` (`aes/funcdef.h:46`).
pub const INQ_TEXT_ATTRIBUTES: i16 = 38;
/// `vst_alignment` (VDI 39, `vdi/vdi_defs.h:383`).
// NEWTOS: not in `aes/funcdef.h` (the AES does not align text); named in its form.
pub const SET_TEXT_ALIGNMENT: i16 = 39;
/// `v_opnvwk` (VDI 100, `vdi/vdi_main.c:75`).
// NEWTOS: not in `aes/funcdef.h` (the AES opens a physical workstation only).
pub const OPEN_VWORKSTATION: i16 = 100;
/// `v_clsvwk` (VDI 101).
// NEWTOS: not in `aes/funcdef.h`, as `OPEN_VWORKSTATION`.
pub const CLOSE_VWORKSTATION: i16 = 101;
/// `vq_extnd` (`aes/funcdef.h:48`).
pub const EXTENDED_INQUIRE: i16 = 102;
/// `vsl_udsty` (`aes/funcdef.h:54`).
pub const SET_UD_LINE_STYLE: i16 = 113;
/// `vr_recfl` (`aes/funcdef.h:55`).
pub const FILL_RECTANGLE: i16 = 114;
/// `vs_clip` (`aes/funcdef.h:67`).
pub const TEXT_CLIP: i16 = 129;

/// `vswr_mode` replace (`MD_REPLACE`, `include/obdefs.h:41`).
pub const MD_REPLACE: i16 = 1;
/// `vswr_mode` XOR (`MD_XOR`, `include/obdefs.h:43`). The AES's `MD_*`
/// numbering and the VDI's `vswr_mode` numbering are the same one: 1 replace,
/// 2 transparent, 3 XOR, 4 reverse transparent
/// (`tos-api/src/vdi/attributes.rs:32-42`, EmuTOS `gsx_attr`
/// `aes/gemgraf.c:211-212`).
pub const MD_XOR: i16 = 3;
/// `vsf_interior` solid fill (`FIS_SOLID`, `include/obdefs.h:47`).
pub const FIS_SOLID: i16 = 1;
/// Colour index `WHITE` (`include/obdefs.h:109`).
pub const WHITE: i16 = 0;
/// Colour index `BLACK` (`include/obdefs.h:110`).
pub const BLACK: i16 = 1;

/// Longest string [`v_gtext`] sends in one call (`intin` capacity).
pub const GTEXT_MAX: usize = 128;
/// Most vertices [`v_pline`] sends in one call (`ptsin` capacity / 2).
pub const PLINE_MAX: usize = 8;

/// One VDI parameter-block call frame.
struct Vdi {
    contrl: [i16; 12],
    intin: [i16; GTEXT_MAX],
    ptsin: [i16; 2 * PLINE_MAX],
    intout: [i16; 45],
    ptsout: [i16; 12],
}

impl Vdi {
    const fn new() -> Self {
        Self { contrl: [0; 12], intin: [0; GTEXT_MAX], ptsin: [0; 2 * PLINE_MAX], intout: [0; 45], ptsout: [0; 12] }
    }

    /// `contrl[0] = op; contrl[1] = n_pts; contrl[3] = n_int;
    /// contrl[6] = handle` — the VDI calling convention (`vdi_main.c`) — then
    /// trap.
    fn call(&mut self, op: i16, n_pts: i16, n_int: i16, handle: i16) {
        self.contrl = [0; 12];
        self.contrl[0] = op;
        self.contrl[1] = n_pts;
        self.contrl[3] = n_int;
        self.contrl[6] = handle;
        let pb: [u64; 5] = [
            self.contrl.as_mut_ptr() as u64,
            self.intin.as_mut_ptr() as u64,
            self.ptsin.as_mut_ptr() as u64,
            self.intout.as_mut_ptr() as u64,
            self.ptsout.as_mut_ptr() as u64,
        ];
        // SAFETY: GEM trap, selector 115 = VDI. The handler reads the block
        // and the input arrays and writes `contrl`/`intout`/`ptsout`; every
        // register but `x0` is preserved. An exception from EL1 (`SPSel = 0`)
        // or EL0 runs the handler on `SP_EL1` (`nostack`).
        unsafe {
            core::arch::asm!(
                "svc #2",
                inout("x0") 115u64 => _,
                in("x1") pb.as_ptr() as u64,
                options(nostack),
            );
        }
    }
}

// ── control ────────────────────────────────────────────────────────────────

/// What [`v_opnvwk`] reports: the handle and the device size
/// (`work_out[0]`/`[1]` = maximum x/y, `docs/api/vdi/workstation/
/// 100-v_opnvwk.md`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Workstation {
    /// The new virtual workstation's handle.
    pub handle: i16,
    /// `work_out[0]`: the largest x coordinate.
    pub xmax: i16,
    /// `work_out[1]`: the largest y coordinate.
    pub ymax: i16,
}

/// `v_opnvwk(work_in, &handle, work_out)` (100) on the AES's physical
/// workstation `phys` (from `graf_handle`). `work_in` = the customary
/// `{1,1,1,1,1,1,1,1,1,1,2}` (defaults, raster coordinates). `None` when
/// the VDI hands back handle 0 (no free slot / no memory).
pub fn v_opnvwk(phys: i16) -> Option<Workstation> {
    let mut v = Vdi::new();
    for (i, w) in v.intin.iter_mut().take(11).enumerate() {
        *w = if i == 10 { 2 } else { 1 };
    }
    v.call(OPEN_VWORKSTATION, 0, 11, phys);
    let handle = v.contrl[6];
    if handle <= 0 {
        return None;
    }
    Some(Workstation { handle, xmax: v.intout[0], ymax: v.intout[1] })
}

/// `v_clsvwk(handle)` (101).
pub fn v_clsvwk(handle: i16) {
    let mut v = Vdi::new();
    v.call(CLOSE_VWORKSTATION, 0, 0, handle);
}

/// `vs_clip(handle, clip_flag, pxyarray)` (129): clip to the corners
/// `pxy = [x1, y1, x2, y2]` when `clip_flag` is non-zero, else switch
/// clipping off. The corners are sent either way, as the classic binding sends
/// its array.
pub fn vs_clip(handle: i16, clip_flag: i16, pxy: [i16; 4]) {
    let mut v = Vdi::new();
    v.intin[0] = clip_flag;
    v.ptsin[..4].copy_from_slice(&pxy);
    v.call(TEXT_CLIP, 2, 1, handle);
}

/// The corners `[x1, y1, x2, y2]` of `r`, the form [`vs_clip`] and
/// [`vr_recfl`] take — the computation of EmuTOS `gsx_sclip`
/// (`aes/gemgraf.c:94-98`).
pub fn grect_to_pxy(r: Grect) -> [i16; 4] {
    [r.x, r.y, r.x + r.w - 1, r.y + r.h - 1]
}

// ── attributes ─────────────────────────────────────────────────────────────

/// `vswr_mode(handle, mode)` (32).
pub fn vswr_mode(handle: i16, mode: i16) {
    let mut v = Vdi::new();
    v.intin[0] = mode;
    v.call(SET_WRITING_MODE, 0, 1, handle);
}

/// `vsl_color(handle, color)` (17).
pub fn vsl_color(handle: i16, color: i16) {
    let mut v = Vdi::new();
    v.intin[0] = color;
    v.call(SET_LINE_COLOR, 0, 1, handle);
}

/// `vsl_udsty(handle, pattern)` (113): the 16-bit user line-style mask
/// (`0xffff` = solid).
pub fn vsl_udsty(handle: i16, pattern: u16) {
    let mut v = Vdi::new();
    v.intin[0] = pattern as i16;
    v.call(SET_UD_LINE_STYLE, 0, 1, handle);
}

/// `vsf_interior(handle, style)` (23).
pub fn vsf_interior(handle: i16, style: i16) {
    let mut v = Vdi::new();
    v.intin[0] = style;
    v.call(SET_FILL_INTERIOR, 0, 1, handle);
}

/// `vsf_color(handle, color)` (25).
pub fn vsf_color(handle: i16, color: i16) {
    let mut v = Vdi::new();
    v.intin[0] = color;
    v.call(SET_FILL_COLOR, 0, 1, handle);
}

/// `vst_color(handle, color)` (22).
pub fn vst_color(handle: i16, color: i16) {
    let mut v = Vdi::new();
    v.intin[0] = color;
    v.call(SET_TEXT_COLOR, 0, 1, handle);
}

/// `vst_alignment(handle, hor, vert, ...)` (39). `vert = 5` = top line.
pub fn vst_alignment(handle: i16, hor: i16, vert: i16) {
    let mut v = Vdi::new();
    v.intin[0] = hor;
    v.intin[1] = vert;
    v.call(SET_TEXT_ALIGNMENT, 0, 2, handle);
}

/// What [`vst_height`] reports: `PTSOUT[0..4]`
/// (`docs/api/vdi/`, EmuTOS `gsx_textsize` reading of it,
/// `aes/gemgsxif.c:590-600`).
#[derive(Clone, Copy, Debug, Default)]
pub struct TextSize {
    /// Character width.
    pub char_w: i16,
    /// Character height.
    pub char_h: i16,
    /// Character cell width.
    pub cell_w: i16,
    /// Character cell height.
    pub cell_h: i16,
}

/// `vst_height(handle, height, &char_w, &char_h, &cell_w, &cell_h)` (12): set
/// the text height to `height` (`PTSIN[1]`) and report the resulting sizes.
pub fn vst_height(handle: i16, height: i16) -> TextSize {
    let mut v = Vdi::new();
    v.ptsin[1] = height;
    v.call(SET_CHAR_HEIGHT, 1, 0, handle);
    TextSize { char_w: v.ptsout[0], char_h: v.ptsout[1], cell_w: v.ptsout[2], cell_h: v.ptsout[3] }
}

// ── inquiry ────────────────────────────────────────────────────────────────

/// `vq_extnd(handle, owflag, work_out)` (102): the 45 `INTOUT` words followed by
/// the 12 `PTSOUT` words, as the classic binding lays them into `work_out[57]`
/// (`work_out[45 + i]` = `PTSOUT[i]`; with `owflag == 0` `PTSOUT[1]` is the
/// workstation's minimum character height `SIZ_TAB[1]`,
/// `vdi/vdi_control.c:626-639`).
pub fn vq_extnd(handle: i16, owflag: i16) -> [i16; 57] {
    let mut v = Vdi::new();
    v.intin[0] = owflag;
    v.call(EXTENDED_INQUIRE, 0, 1, handle);
    let mut out = [0i16; 57];
    out[..45].copy_from_slice(&v.intout);
    out[45..].copy_from_slice(&v.ptsout);
    out
}

/// `vqt_attributes(handle, attrib)` (38): the text attributes as the classic
/// binding lays them into `attrib[10]` — `INTOUT[0..6]`, then `PTSOUT[0..4]`
/// (`attrib[7]` = `PTSOUT[1]`, the current character height; EmuTOS
/// `gsx_textsize`, `aes/gemgsxif.c:505-512`).
pub fn vqt_attributes(handle: i16) -> [i16; 10] {
    let mut v = Vdi::new();
    v.call(INQ_TEXT_ATTRIBUTES, 0, 0, handle);
    let mut out = [0i16; 10];
    out[..6].copy_from_slice(&v.intout[..6]);
    out[6..].copy_from_slice(&v.ptsout[..4]);
    out
}

// ── output ─────────────────────────────────────────────────────────────────

/// `v_pline(handle, count, pxyarray)` (6): the polyline through the vertices
/// `pxy = [x0, y0, x1, y1, …]`; at most [`PLINE_MAX`] vertices (a longer array
/// is cut there, an odd trailing word ignored).
pub fn v_pline(handle: i16, pxy: &[i16]) {
    let mut v = Vdi::new();
    let n = (pxy.len() / 2).min(PLINE_MAX);
    v.ptsin[..2 * n].copy_from_slice(&pxy[..2 * n]);
    v.call(POLYLINE, n as i16, 0, handle);
}

/// `vr_recfl(handle, pxyarray)` (114): fill the rectangle with corners
/// `pxy = [x1, y1, x2, y2]` with the fill attributes, no perimeter.
pub fn vr_recfl(handle: i16, pxy: [i16; 4]) {
    let mut v = Vdi::new();
    v.ptsin[..4].copy_from_slice(&pxy);
    v.call(FILL_RECTANGLE, 2, 0, handle);
}

/// `v_gtext(handle, x, y, text)` (8): one character per `intin` word (the
/// byte value, as gemlib's `vdi_str2array` widens it); at most
/// [`GTEXT_MAX`] characters.
pub fn v_gtext(handle: i16, x: i16, y: i16, text: &[u8]) {
    let mut v = Vdi::new();
    let n = text.len().min(GTEXT_MAX);
    for (d, &c) in v.intin.iter_mut().zip(&text[..n]) {
        *d = i16::from(c);
    }
    v.ptsin[0] = x;
    v.ptsin[1] = y;
    v.call(TEXT, 1, n as i16, handle);
}
