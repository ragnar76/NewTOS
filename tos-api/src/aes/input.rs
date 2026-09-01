//! AES input layer — mouse ownership and the button-delay window.
//!
//! Source: EmuTOS `aes/geminput.c`. This is the small part of that file our
//! architecture needs: the state the Event Manager consults (`gl_mowner`,
//! `button`, `xrat`/`yrat`, `gl_bdely`) and the two interrupt-side handlers
//! that maintain it, `b_click` (`:110-140`) and `mchange` (`:360-373`).
//!
//! **How EmuTOS gets here.** The AES installs its own handlers into the VDI's
//! mouse-interrupt vectors — `gsx_setmb(far_bcha, far_mcha, &drwaddr)`
//! (`aes/gemgsxif.c:271`) is `vex_butv`/`vex_motv`. The VDI's `mouse_int` then
//! calls `user_but` on a button change and `user_mot` on movement, the latter
//! with the **clipped** new position (`vdi/vdi_asm.S:271-284` and `:339-362`);
//! `far_bcha` calls `b_click`, `far_mcha` forks `mchange`
//! (`aes/gemdosif.S:306-347`). Our equivalent of `mouse_int` is
//! [`crate::vdi::feed_relative`], which calls [`b_click`] and [`mchange`] at the
//! same two points and with the same arguments.
//!
//! **What is *not* reproduced, and why.** EmuTOS's `bchange`/`b_delay` deliver
//! the button change through the fork queue into a per-process event-block list
//! (`post_button`, `:462-483`); our Event Manager polls the live VDI button byte
//! instead, so the delivery half has no counterpart. What *is* observable
//! through that difference — which process may act on the sampled level, and how
//! long a click window stays open — is exactly what this module keeps.
//!
//! Single boot CPU (the AES is entered only from the GEM trap, these two hooks
//! only from the IKBD decode path), so plain `Relaxed` atomics suffice, matching
//! the rest of the AES state.

use core::sync::atomic::{AtomicBool, AtomicI16, AtomicU8, Ordering};

/// EmuTOS `MB_DOWN` (`aes/geminput.c:46`) — the left button, the only state
/// that moves mouse ownership.
const MB_DOWN: u8 = 0x01;

/// EmuTOS `button` — the button state the AES has acted on. Maintained by
/// `bchange` (`aes/geminput.c:342`); here by [`b_click`], which runs
/// `bchange`'s ownership decision inline because our Event Manager has no fork
/// queue to defer it to.
static BUTTON: AtomicU8 = AtomicU8::new(0);

/// EmuTOS `xrat`/`yrat` — the **last reported** (clipped) pointer position, the
/// baseline `mchange` measures its ±2 threshold against (`aes/geminput.c:362`).
/// Deliberately not `crate::vdi::mouse_position()`: that is the *current*
/// position, which by the time `mchange` runs is already the new one.
static XRAT: AtomicI16 = AtomicI16::new(0);
static YRAT: AtomicI16 = AtomicI16::new(0);

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

/// EmuTOS `gl_bdely != 0` — a button-delay window is open: the interval in
/// which a further click still joins the current gesture
/// (`aes/geminput.c:59-60`). EmuTOS counts it down in ticks; our Event Manager
/// holds the deadline in `count_clicks`, so the shared state is just "open",
/// and `b_delay(gl_bdely)` — the forced expiry `mchange` performs — is
/// [`b_delay`] clearing it.
static GL_BDELY: AtomicBool = AtomicBool::new(false);

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

/// `user_mot` — the AES's motion handler, EmuTOS `mchange`
/// (`aes/geminput.c:360-373`). `rx`/`ry` are the new **clipped** pointer
/// position, as the VDI passes them (`vdi/vdi_asm.S:355-362`); the VDI calls
/// this only for a packet that actually carried movement (`:305-320`).
///
/// Two things happen, in EmuTOS's order:
///
/// 1. *"zero out button wait if mouse moves more than a little"* — a move of
///    more than ±2 in either axis against the previous reported position forces
///    the pending button delay to expire immediately, `b_delay(gl_bdely)`
///    (`:365-368`). The clicks counted so far are delivered at once; a click
///    after the move starts a new gesture instead of completing a double click.
/// 2. The pointer entering the menu bar with the buttons up hands the mouse to
///    the Control Manager (`:387-389`), which is how a hover menu takes over
///    without a click. `gl_ctwait` is the menu tree's `THEACTIVE` rectangle
///    (`aes/gemmnlib.c:717`), the menu-bar row — [`menu::desk_top`] here, the
///    same row [`event::menu_intercept`] tracks.
// The threshold test is EmuTOS's line for line (`aes/geminput.c:366-367`);
// folding it into two range checks would read further from the source than it
// is worth.
#[allow(clippy::manual_range_contains)]
pub(crate) fn mchange(rx: i16, ry: i16) {
    let dx = XRAT.load(Ordering::Relaxed) - rx;
    let dy = YRAT.load(Ordering::Relaxed) - ry;
    if bdely_pending() && (dx > 2 || dx < -2 || dy > 2 || dy < -2) {
        b_delay();
    }

    // "xrat, yrat hold true" (aes/geminput.c:371)
    XRAT.store(rx, Ordering::Relaxed);
    YRAT.store(ry, Ordering::Relaxed);

    if GL_MOWNER_APP.load(Ordering::Relaxed)
        && BUTTON.load(Ordering::Relaxed) == 0
        && !super::menu::active().is_null()
        && ry < super::menu::desk_top()
    {
        GL_MOWNER_APP.store(false, Ordering::Relaxed);
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

/// Open the button-delay window — EmuTOS `gl_bdely = gl_dclick` in `b_click`
/// (`aes/geminput.c:140`). EmuTOS opens it only when someone is waiting for
/// more than one click *and* the change is a press (`if (gl_bpend && state)`,
/// `:133`); the caller applies both conditions.
pub(super) fn b_arm() {
    GL_BDELY.store(true, Ordering::Relaxed);
}

/// Close the button-delay window normally — the `gl_bdely` countdown reaching
/// zero (`b_delay`, `aes/geminput.c:155-171`), here the counting loop finishing.
pub(super) fn b_disarm() {
    GL_BDELY.store(false, Ordering::Relaxed);
}

/// Is a button-delay window open? EmuTOS `if (gl_bdely)`
/// (`aes/geminput.c:117,366`).
pub(super) fn bdely_pending() -> bool {
    GL_BDELY.load(Ordering::Relaxed)
}

/// `b_delay(gl_bdely)` — subtract the whole remaining delay, so it expires now
/// (`aes/geminput.c:155-171`, called that way from `mchange`, `:368`). The
/// clicks accumulated so far are what the waiter gets.
fn b_delay() {
    GL_BDELY.store(false, Ordering::Relaxed);
}
