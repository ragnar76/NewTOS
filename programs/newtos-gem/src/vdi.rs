//! VDI trap binding (`svc #2`, selector 115) — a virtual workstation and the
//! few primitives a window's work area needs (clip, fill, text).
//!
//! ABI (`docs/sdk/05-abi-pitfalls.md` §A1; `sdk/gemlib/overlay/_gc_vdi.c`):
//! `x0` = 115, `x1` = the VDI parameter block — five consecutive native
//! pointers `contrl`, `intin`, `ptsin`, `intout`, `ptsout`; the dispatcher
//! picks the function from `contrl[0]` and returns `flip_y` in `x0`, which
//! no caller here needs. The pattern is the kernel desktop's
//! `kernel/src/desk/vdibind.rs` `Vdi::call`.
//!
//! Opcodes and argument layouts: `docs/api/vdi/` (EmuTOS `vdi/vdi_main.c`
//! jump table; the NewTOS dispatch table `tos-api/src/vdi/mod.rs`).

use crate::aes::Grect;

const V_GTEXT: i16 = 8;
const VST_COLOR: i16 = 22;
const VSF_INTERIOR: i16 = 23;
const VSF_COLOR: i16 = 25;
const VSWR_MODE: i16 = 32;
const VST_ALIGNMENT: i16 = 39;
const V_OPNVWK: i16 = 100;
const V_CLSVWK: i16 = 101;
const VR_RECFL: i16 = 114;
const VS_CLIP: i16 = 129;

/// `vswr_mode` replace (`MD_REPLACE`).
pub const MD_REPLACE: i16 = 1;
/// `vsf_interior` solid fill (`FIS_SOLID`).
pub const FIS_SOLID: i16 = 1;
/// Colour indices `WHITE`/`BLACK` (`obdefs.h`).
pub const WHITE: i16 = 0;
pub const BLACK: i16 = 1;

/// Longest string [`v_gtext`] sends in one call (`intin` capacity).
pub const GTEXT_MAX: usize = 128;

/// One VDI parameter-block call frame.
struct Vdi {
    contrl: [i16; 12],
    intin: [i16; GTEXT_MAX],
    ptsin: [i16; 8],
    intout: [i16; 45],
    ptsout: [i16; 12],
}

impl Vdi {
    const fn new() -> Self {
        Self { contrl: [0; 12], intin: [0; GTEXT_MAX], ptsin: [0; 8], intout: [0; 45], ptsout: [0; 12] }
    }

    /// `contrl[0] = op; contrl[1] = n_pts; contrl[3] = n_int;
    /// contrl[6] = handle`, then trap.
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
        // register but `x0` is preserved.
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

/// What [`v_opnvwk`] reports: the handle and the device size
/// (`work_out[0]`/`[1]` = maximum x/y, `docs/api/vdi/workstation/
/// 100-v_opnvwk.md`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Workstation {
    pub handle: i16,
    pub xmax: i16,
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
    v.call(V_OPNVWK, 0, 11, phys);
    let handle = v.contrl[6];
    if handle <= 0 {
        return None;
    }
    Some(Workstation { handle, xmax: v.intout[0], ymax: v.intout[1] })
}

/// `v_clsvwk(handle)` (101).
pub fn v_clsvwk(handle: i16) {
    let mut v = Vdi::new();
    v.call(V_CLSVWK, 0, 0, handle);
}

/// `vs_clip(handle, on, r)` (129): clip to `r`, or off for `None`.
pub fn vs_clip(handle: i16, r: Option<Grect>) {
    let mut v = Vdi::new();
    if let Some(r) = r {
        v.intin[0] = 1;
        v.ptsin[0] = r.x;
        v.ptsin[1] = r.y;
        v.ptsin[2] = r.x + r.w - 1;
        v.ptsin[3] = r.y + r.h - 1;
    }
    v.call(VS_CLIP, 2, 1, handle);
}

/// `vswr_mode(handle, mode)` (32).
pub fn vswr_mode(handle: i16, mode: i16) {
    let mut v = Vdi::new();
    v.intin[0] = mode;
    v.call(VSWR_MODE, 0, 1, handle);
}

/// `vsf_interior(handle, style)` (23).
pub fn vsf_interior(handle: i16, style: i16) {
    let mut v = Vdi::new();
    v.intin[0] = style;
    v.call(VSF_INTERIOR, 0, 1, handle);
}

/// `vsf_color(handle, color)` (25).
pub fn vsf_color(handle: i16, color: i16) {
    let mut v = Vdi::new();
    v.intin[0] = color;
    v.call(VSF_COLOR, 0, 1, handle);
}

/// `vst_color(handle, color)` (22).
pub fn vst_color(handle: i16, color: i16) {
    let mut v = Vdi::new();
    v.intin[0] = color;
    v.call(VST_COLOR, 0, 1, handle);
}

/// `vst_alignment(handle, hor, vert, ...)` (39). `vert = 5` = top line.
pub fn vst_alignment(handle: i16, hor: i16, vert: i16) {
    let mut v = Vdi::new();
    v.intin[0] = hor;
    v.intin[1] = vert;
    v.call(VST_ALIGNMENT, 0, 2, handle);
}

/// `vr_recfl(handle, r)` (114): fill `r` with the fill attributes, no
/// perimeter.
pub fn vr_recfl(handle: i16, r: Grect) {
    let mut v = Vdi::new();
    v.ptsin[0] = r.x;
    v.ptsin[1] = r.y;
    v.ptsin[2] = r.x + r.w - 1;
    v.ptsin[3] = r.y + r.h - 1;
    v.call(VR_RECFL, 2, 0, handle);
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
    v.call(V_GTEXT, 1, n as i16, handle);
}
