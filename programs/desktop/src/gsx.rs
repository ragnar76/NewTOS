//! `desktop::gsx` — the desktop's use of the VDI, the third of its system
//! interfaces next to the AES (`svc #2`, selector 200) and GEMDOS (`svc #1`).
//! The trap bindings themselves are [`newtos_gem::vdi`] (DESK43 U6b moved them
//! out of this file); what stays here is desktop logic on top of them — the
//! workstation handle the desktop learned, the small-font query, and the
//! `gsx_*` drawing helpers EmuTOS's ROM desktop borrows from the AES. The
//! module is named after those helpers; until DESK43 U8b it was
//! `kernel/src/desk/vdibind.rs`, a name it outgrew when the binding left.
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

use newtos_gem::vdi;

use super::global::Point;
use tos_types::aes::Grect;

/// A solid 16-bit user line-style mask (`aes/gemgraf.c:162,421`).
const SOLID_MASK: u16 = 0xffff;

/// The AES's workstation handle as the desktop learned it from
/// `graf_handle` (`deskmain.c:1956`'s `gl_handle`), or 0 while it has not
/// been asked yet. Set by [`set_handle`] at the start of every `deskmain`
/// run and cleared by [`reset`] when the AES is restarted under us.
static HANDLE: AtomicI16 = AtomicI16::new(0);

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
    // `work_out[46]` = `PTSOUT[1]` = `SIZ_TAB[1]`, the minimum char height.
    let ws_chminh = vdi::vq_extnd(h, 0)[46];
    // `attrib[7]` = `PTSOUT[1]`, the current char height.
    let ws_chmaxh = vdi::vqt_attributes(h)[7];
    let small = vdi::vst_height(h, ws_chminh);
    vdi::vst_height(h, ws_chmaxh);
    (small.cell_w, small.cell_h)
}

/// `gsx_sclip(pt)` (`aes/gemgraf.c:91-105`) — set the workstation clip
/// rectangle to `r`, or switch clipping off when `r` is degenerate.
pub(super) fn gsx_sclip(r: Grect) {
    let h = handle();
    if h <= 0 {
        return;
    }
    let on = r.w != 0 && r.h != 0;
    vdi::vs_clip(h, i16::from(on), vdi::grect_to_pxy(r));
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
    vdi::vswr_mode(h, mode);
    vdi::vsl_color(h, color);
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
        vdi::vsl_udsty(h, st);
        vdi::v_pline(h, &[x0, y0, x1, y1]);
    }
    vdi::vsl_udsty(h, SOLID_MASK);
}
