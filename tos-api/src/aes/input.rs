//! AES input layer — mouse ownership.
//!
//! Source: EmuTOS `aes/geminput.c`. This is the small part of that file our
//! architecture needs: the state the Event Manager consults (`gl_mowner`,
//! `button`) and the interrupt-side handler that maintains it, `b_click`
//! (`:110-140`).
//!
//! **How EmuTOS gets here.** The AES installs its own handlers into the VDI's
//! mouse-interrupt vectors — `gsx_setmb(far_bcha, far_mcha, &drwaddr)`
//! (`aes/gemgsxif.c:271`) is `vex_butv`/`vex_motv`. The VDI's `mouse_int` then
//! calls `user_but` on a button change (`vdi/vdi_asm.S:271-284`), and
//! `far_bcha` calls `b_click` (`aes/gemdosif.S:306-325`). Our equivalent of
//! `mouse_int` is [`crate::vdi::feed_relative`], which calls [`b_click`] at the
//! same point and with the same argument.
//!
//! **What is *not* reproduced, and why.** EmuTOS's `bchange` delivers the button
//! change through the fork queue into a per-process event-block list
//! (`post_button`, `:462-483`); our Event Manager polls the live VDI button byte
//! instead, so the delivery half has no counterpart. What *is* observable
//! through that difference — which process may act on the sampled level — is
//! exactly what this module keeps.
//!
//! Single boot CPU (the AES is entered only from the GEM trap, this hook only
//! from the IKBD decode path), so plain `Relaxed` atomics suffice, matching the
//! rest of the AES state.

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// EmuTOS `MB_DOWN` (`aes/geminput.c:46`) — the left button, the only state
/// that moves mouse ownership.
const MB_DOWN: u8 = 0x01;

/// EmuTOS `button` — the button state the AES has acted on. Maintained by
/// `bchange` (`aes/geminput.c:342`); here by [`b_click`], which runs
/// `bchange`'s ownership decision inline because our Event Manager has no fork
/// queue to defer it to.
static BUTTON: AtomicU8 = AtomicU8::new(0);

/// EmuTOS `gl_mowner == rlr` — `true` while the running application owns the
/// mouse, `false` while the Control Manager does.
///
/// EmuTOS holds an `AESPD *`; with one AES application the only two values it
/// can take are "the app" and `ctl_pd`, so a flag carries the whole state.
/// Initially the app: `geminit.c:888` starts `gl_mowner` at `ctl_pd`, but the
/// Control Manager's very first loop pass calls `w_setactive` →
/// `ct_chgown(D.w_win[w_top()].w_owner, …)` → `set_mown` before anything can
/// observe it (`aes/gemctrl.c:441`), and at single-app that owner is the
/// application.
static GL_MOWNER_APP: AtomicBool = AtomicBool::new(true);

// ── the two VDI mouse-interrupt vectors ────────────────────────────────────

/// `user_but` — the AES's button-change handler, EmuTOS `b_click`
/// (`aes/geminput.c:110-140`) plus the ownership decision `bchange` makes from
/// it (`:333-335`).
///
/// `new` is the new button bitmask in VDI convention (bit 0 = left), the `d0`
/// the VDI passes to `user_but` (`vdi/vdi_asm.S:270-284`). Like `b_click` this
/// ignores anything that is not a change of state.
///
/// On the 0 → left-down edge `bchange` recomputes the mouse owner from the
/// pointer position: `if (!gl_ctmown && (new == MB_DOWN) && (button == 0))
/// gl_mowner = chkown();` — and `chkown` (`:310-322`) resolves
/// [`window::chk_ctrl`](super::window::chk_ctrl) to the active process (`+1`),
/// the Control Manager (`-1`) or the desktop window's owner (`0`); at
/// single-app the first and last are both this application. The owner is
/// latched here, at the press, and does not follow the pointer while the button
/// is held — that is what lets a drag start in a window and leave it.
///
/// `gl_ctmown` (the Control Manager holding the mouse *form* across a menu
/// track, `aes/gemctrl.c:400-417`) has no counterpart: our menu tracker runs
/// synchronously inside the trap, so no event loop can observe the interval.
pub(crate) fn b_click(new: u8) {
    // "ignore it unless it represents a change" (aes/geminput.c:113)
    let prev = BUTTON.swap(new, Ordering::Relaxed);
    if new == prev {
        return;
    }
    if new == MB_DOWN && prev == 0 {
        let (x, y) = crate::vdi::mouse_position();
        // chkown: +1 → gl_cowner, 0 → D.w_win[0].w_owner (both the app at
        // single-app), −1 → ctl_pd.
        GL_MOWNER_APP.store(super::window::chk_ctrl(x, y) >= 0, Ordering::Relaxed);
    }
}

// ── what the Event Manager consults ────────────────────────────────────────

/// EmuTOS `rlr == gl_mowner` (`aes/gemevlib.c:185`) — may this application run
/// `ev_multi`'s quick checks?
pub(super) fn own_mouse() -> bool {
    GL_MOWNER_APP.load(Ordering::Relaxed)
}

/// The Control Manager's `w_setactive()` (`aes/gemctrl.c:441`): give the mouse
/// to the owner of the top window — `ct_chgown(ppd, &d)` → `set_mown`
/// (`aes/gemwmlib.c:326-333`, `aes/gemctrl.c:392-396`). `set_mown` changes
/// nothing while a button is held (`if (!button)`, `aes/geminput.c:222`), so a
/// press latched to the Control Manager stays with it until the release. At
/// single-app the top window's owner is always this application.
pub(super) fn w_setactive() {
    if BUTTON.load(Ordering::Relaxed) == 0 {
        GL_MOWNER_APP.store(true, Ordering::Relaxed);
    }
}
