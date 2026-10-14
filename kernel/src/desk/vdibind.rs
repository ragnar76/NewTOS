//! `desk::vdibind` — the VDI trap binding, the third and last of the
//! desktop's system interfaces next to [`super`]'s aesbind (`svc #2`,
//! selector 200) and [`super::dosbind`] (`svc #1`).
//!
//! **Why the desktop needs the VDI at all.** Exactly one desktop gesture
//! draws without going through an object tree: the drag outline
//! `gr_drgplns` complements over the screen while a selection is being
//! dragged (`deskact.c:248-355`). EmuTOS draws it with the AES's internal
//! `gsx_*` helpers — `gsx_sclip`, `gsx_attr`, `gsx_pline`
//! (`aes/gemgraf.c:91-105,140-182,201-240`) — which are plain VDI calls on
//! the workstation handle `gl_handle`. There the desktop may reach them
//! because the ROM desktop is linked into the AES; here it may not, so the
//! same calls are made where a GEM application makes them: over the VDI
//! trap.
//!
//! The one other thing the desktop takes from the VDI is the small font's
//! cell `gl_wschar`/`gl_hschar` ([`small_font_cell`]), once per `deskmain`
//! run: the ROM desktop reads the AES's global, which an application cannot.
//!
//! **The workstation is the AES's own.** EmuTOS's desktop draws on the
//! AES's workstation `gl_handle`, which it learns from `graf_handle`
//! (`deskmain.c:1956`) — `gr_bwait` even carries the comment "Since the
//! desktop and the AES currently share the same VDI workstation, we have to
//! reset the clipping and drawing mode here" (`deskact.c:212-215`). NewTOS's
//! AES opens that physical workstation at session start
//! (`tos_api::aes::gsxif::gsx_init`, EmuTOS `gsx_wsopen`,
//! `aes/gemgsxif.c:169-186`), so this module draws on the `graf_handle`
//! handle directly and opens no workstation of its own (K1, 2026-10-07; it
//! used to open a virtual one while handle 1 was never opened). The line
//! style `gsx_xline` relies on — `vsl_type(7)` with a solid
//! `vsl_udsty(0xffff)` — is the AES's set-up of that workstation
//! (`gsx_start`, `aes/gemgraf.c:420-422`), not this module's.

use core::sync::atomic::{AtomicI16, Ordering};

use super::global::Point;
use tos_types::aes::Grect;

// VDI opcodes (`tos-api/src/vdi/mod.rs` dispatch table).
const OP_V_PLINE: i16 = 6;
const OP_VST_HEIGHT: i16 = 12;
const OP_VSL_COLOR: i16 = 17;
const OP_VQT_ATTRIBUTES: i16 = 38;
const OP_VQ_EXTND: i16 = 102;
const OP_VSWR_MODE: i16 = 32;
const OP_VSL_UDSTY: i16 = 113;
const OP_VS_CLIP: i16 = 129;

/// `MD_XOR` (`include/aesdefs.h`) = 3. `gsx_attr` passes the AES's `MD_*`
/// number straight into the VDI's `SET_WRITING_MODE`
/// (`aes/gemgraf.c:211-212`), and the VDI's `vswr_mode` numbering is the same
/// one: 1 replace, 2 transparent, **3 XOR**, 4 reverse transparent
/// (`tos-api/src/vdi/attributes.rs:32-42`).
pub(super) const MD_XOR: i16 = 3;
/// `BLACK` (`obdefs.h:44`) — the pen `gr_bwait` complements with.
pub(super) const BLACK: i16 = 1;
/// A solid 16-bit user line-style mask (`aes/gemgraf.c:162,421`).
const SOLID_MASK: i16 = -1; // 0xffff

/// The AES's workstation handle as the desktop learned it from
/// `graf_handle` (`deskmain.c:1956`'s `gl_handle`), or 0 while it has not
/// been asked yet. Set by [`set_handle`] at the start of every `deskmain`
/// run and cleared by [`reset`] when the AES is restarted under us.
static HANDLE: AtomicI16 = AtomicI16::new(0);

/// Issue the VDI trap (`svc #2`): selector 115 in `x0`, the five-pointer VDI
/// parameter block in `x1` (`kernel/src/arch/aarch64/exceptions.rs:382`).
fn vdi_trap(vdipb: u64) {
    // SAFETY: VDI trap; results are written back into the parameter block.
    unsafe {
        core::arch::asm!(
            "svc #2",
            inout("x0") 115u64 => _,
            in("x1") vdipb,
            options(nostack),
        );
    }
}

/// A reusable VDI parameter block. [`Vdi::call`] lays the four `CONTRL`
/// words every VDI binding writes — opcode, `PTSIN` vertex count, `INTIN`
/// word count, workstation handle — and traps.
struct Vdi {
    contrl: [i16; 12],
    intin: [i16; 16],
    ptsin: [i16; 4],
    intout: [i16; 45],
    ptsout: [i16; 12],
}

impl Vdi {
    fn new() -> Self {
        Self { contrl: [0; 12], intin: [0; 16], ptsin: [0; 4], intout: [0; 45], ptsout: [0; 12] }
    }

    /// `contrl[0] = opcode; contrl[1] = n_pts; contrl[3] = n_int;
    /// contrl[6] = handle` — the VDI calling convention (`vdi_main.c`).
    fn call(&mut self, op: i16, n_pts: i16, n_int: i16, handle: i16) {
        self.contrl = [0; 12];
        self.contrl[0] = op;
        self.contrl[1] = n_pts;
        self.contrl[3] = n_int;
        self.contrl[6] = handle;
        let block: [u64; 5] = [
            self.contrl.as_ptr() as u64,
            self.intin.as_ptr() as u64,
            self.ptsin.as_ptr() as u64,
            self.intout.as_ptr() as u64,
            self.ptsout.as_ptr() as u64,
        ];
        vdi_trap(block.as_ptr() as u64);
    }
}

/// The AES's workstation handle `gl_handle`, as [`set_handle`] recorded it
/// from `deskmain`'s `graf_handle` (`deskmain.c:1956`).
///
/// Returns 0 when the AES has no workstation (`gl_handle` still 0: no raster
/// backend, so `gsx_init`'s `v_opnwk` never reached a handler), which every
/// wrapper below treats as "draw nothing". Such a board also never gets
/// here: without a raster backend `sh_ldapp`'s `DESKTOP_APP` arm runs the
/// console instead of the desktop, so no drag can start.
fn handle() -> i16 {
    HANDLE.load(Ordering::Relaxed).max(0)
}

/// Record `gl_handle` from `deskmain`'s `graf_handle` (`deskmain.c:1956`).
pub(super) fn set_handle(h: i16) {
    HANDLE.store(h, Ordering::Relaxed);
}

/// Forget the workstation handle. Called from [`super::reset`] on the AES
/// restart path (A106): the desktop is a program that restarts and asks
/// `graf_handle` again (`deskmain.c:1956`) at the start of its next run.
pub(super) fn reset() {
    HANDLE.store(0, Ordering::Relaxed);
}

/// The small font's character cell `(gl_wschar, gl_hschar)`, asked of the
/// AES's workstation exactly as the AES's `gsx_start` computes it
/// (`aes/gemgraf.c:404-410`):
///
/// ```c
/// gsx_textsize(&gl_wptschar, &gl_hptschar, &gl_wchar, &gl_hchar);
/// gl_ws.ws_chmaxh = gl_hptschar;  /* save the IBM font size here */
/// vst_height(gl_ws.ws_chminh, &gl_wsptschar, &gl_hsptschar, &gl_wschar, &gl_hschar);
/// /* must restore the current font height */
/// vst_height(gl_ws.ws_chmaxh, &dummy, &dummy, &dummy, &dummy);
/// ```
///
/// The ROM desktop reads the result straight out of the AES's globals
/// (`deskapp.c:519-520`); an application has no AES call for it, so it makes
/// the same VDI calls on the workstation it shares with the AES
/// (`deskact.c:212-215`). `ws_chminh` is `SIZ_TAB[1]`, which `v_opnwk`
/// returned to the AES in `PTSOUT[1]` (`aes/gemgsxif.c:169-186`) and
/// `vq_extnd(handle, 0)` returns again (`vdi/vdi_control.c:626-639`);
/// `gsx_textsize` is `vqt_attributes` (`aes/gemgsxif.c:505-512`) and
/// `vst_height` answers the cell in `PTSOUT[2..3]`
/// (`aes/gemgsxif.c:590-600`). The second `vst_height` puts the
/// workstation's text height back where it was, so the shared workstation
/// ends in the state it started in.
///
/// `(0, 0)` without a workstation (see [`handle`]).
pub(super) fn small_font_cell() -> (i16, i16) {
    let h = handle();
    if h <= 0 {
        return (0, 0);
    }
    let mut v = Vdi::new();
    v.intin[0] = 0;
    v.call(OP_VQ_EXTND, 0, 1, h);
    let ws_chminh = v.ptsout[1];

    let mut v = Vdi::new();
    v.call(OP_VQT_ATTRIBUTES, 0, 0, h);
    let ws_chmaxh = v.ptsout[1];

    let mut v = Vdi::new();
    v.ptsin[0] = 0;
    v.ptsin[1] = ws_chminh;
    v.call(OP_VST_HEIGHT, 1, 0, h);
    let cell = (v.ptsout[2], v.ptsout[3]);

    let mut v = Vdi::new();
    v.ptsin[0] = 0;
    v.ptsin[1] = ws_chmaxh;
    v.call(OP_VST_HEIGHT, 1, 0, h);
    cell
}

/// `gsx_sclip(pt)` (`aes/gemgraf.c:91-105`) — set the workstation clip
/// rectangle to `r`, or switch clipping off when `r` is degenerate.
pub(super) fn gsx_sclip(r: Grect) {
    let h = handle();
    if h <= 0 {
        return;
    }
    let mut v = Vdi::new();
    let on = r.w != 0 && r.h != 0;
    v.intin[0] = i16::from(on);
    v.ptsin[0] = r.x;
    v.ptsin[1] = r.y;
    v.ptsin[2] = r.x + r.w - 1;
    v.ptsin[3] = r.y + r.h - 1;
    v.call(OP_VS_CLIP, 2, 1, h);
}

/// `gsx_attr(FALSE, mode, color)` (`aes/gemgraf.c:201-240`) — the writing
/// mode and the **line** pen.
///
/// EmuTOS caches both in `gl_mode`/`gl_lcolor` and skips the VDI call when
/// the value is unchanged. That cache is a ROM-era call-count saving with no
/// observable effect (the workstation ends in the same state either way), so
/// it is not reproduced: both are set, every time, which is also the only
/// safe thing to do on a workstation this module does not exclusively own.
pub(super) fn gsx_attr_line(mode: i16, color: i16) {
    let h = handle();
    if h <= 0 {
        return;
    }
    let mut v = Vdi::new();
    v.intin[0] = mode;
    v.call(OP_VSWR_MODE, 0, 1, h);
    let mut v = Vdi::new();
    v.intin[0] = color;
    v.call(OP_VSL_COLOR, 0, 1, h);
}

/// `gsx_pline(offx, offy, cnt, pts)` (`aes/gemgraf.c:170-182`) — draw the
/// polyline `pts`, translated by `(offx, offy)`.
///
/// The segments go through `gsx_xline` (`:140-162`), which is not a plain
/// `v_pline`: each segment is drawn with a 50 %-dotted user line style whose
/// phase is chosen from the segment's own coordinate parity — `verttbl` for a
/// vertical segment, `hztltbl` for a horizontal one. That is what makes the
/// dotted outline **position-independent**, so complementing the same
/// polyline a second time restores every pixel it took, which is the whole
/// erase mechanism of the drag. The solid mask goes back at the end.
pub(super) fn gsx_pline(offx: i16, offy: i16, pts: &[Point]) {
    let h = handle();
    if h <= 0 || pts.len() < 2 {
        return;
    }
    /// `hztltbl` (`aes/gemgraf.c:142`) — phase by the y parity of a
    /// horizontal segment.
    const HZTLTBL: [u16; 2] = [0x5555, 0xaaaa];
    /// `verttbl` (`:143`) — phase by the x/y parity pair of a vertical one.
    const VERTTBL: [u16; 4] = [0x5555, 0xaaaa, 0xaaaa, 0x5555];

    let mut v = Vdi::new();
    for seg in pts.windows(2) {
        let (x0, y0) = (offx + seg[0].x, offy + seg[0].y);
        let (x1, y1) = (offx + seg[1].x, offy + seg[1].y);
        let st = if x0 == x1 {
            VERTTBL[((x0 & 1) | ((y0 & 1) << 1)) as usize]
        } else {
            // `linexy = (*ppoints < *(ppoints+2)) ? ppoints : ppoints + 2;`
            // — the y of whichever endpoint is further left.
            let ly = if x0 < x1 { y0 } else { y1 };
            HZTLTBL[(ly & 1) as usize]
        };
        v.intin[0] = st as i16;
        v.call(OP_VSL_UDSTY, 0, 1, h);
        v.ptsin[0] = x0;
        v.ptsin[1] = y0;
        v.ptsin[2] = x1;
        v.ptsin[3] = y1;
        v.call(OP_V_PLINE, 2, 0, h);
    }
    v.intin[0] = SOLID_MASK;
    v.call(OP_VSL_UDSTY, 0, 1, h);
}
