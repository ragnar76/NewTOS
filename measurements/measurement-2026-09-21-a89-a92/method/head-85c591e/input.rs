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
//! (`post_button`, `:463-483`); our Event Manager polls the live VDI button byte
//! instead, so the delivery half has no counterpart. What *is* observable
//! through that difference — which process may act on the sampled level, and how
//! long a click window stays open — is exactly what this module keeps.
//!
//! **The Control Manager's two locks over the mouse.** Beside `gl_mowner` this
//! module also carries the Control Manager's `gl_ctmown` (`aes/gemctrl.c:50`),
//! the flag `mn_do` raises for the whole of a menu-bar track (`ct_mouse(TRUE)`
//! / `ct_mouse(FALSE)`, `aes/gemmnlib.c:277,513`; `aes/gemctrl.c:400-417`) so
//! that a press during the track does not move the mouse away from `ctl_pd`
//! (`bchange`, `aes/geminput.c:334-335`; `ct_chgown`, `aes/gemctrl.c:395`) —
//! and the state of `ct_msgup`'s `while (button & 0x0001) dsptch();`
//! (`aes/gemctrl.c:100-101`), the wait that keeps the Control Manager from
//! looping again until the press that selected a menu item has been released.
//! Together they are the contract of `docs/api/aes/menu.md` "Mouse ownership
//! over the track": the selecting press and its release reach the application
//! only as the one `MN_SELECTED`, never as `MU_BUTTON`.
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

/// EmuTOS `gl_ctmown` (`aes/gemctrl.c:50`) — the Control Manager holds the
/// mouse across a menu-bar track. Raised and lowered by [`ct_mouse`] from
/// [`menu::track`](super::menu::track), EmuTOS `mn_do`'s `ct_mouse(TRUE)` on
/// entry and `ct_mouse(FALSE)` on every exit (`aes/gemmnlib.c:277,513`). While
/// set, a fresh button-down does **not** re-assign the owner ([`b_click`],
/// `bchange`'s `if (!gl_ctmown && …)`, `aes/geminput.c:334-335`) and the
/// Control Manager's loop head does not hand the mouse back ([`w_setactive`],
/// `ct_chgown`'s `if (!gl_ctmown) set_mown(mpd);`, `aes/gemctrl.c:395-396`).
static GL_CTMOWN: AtomicBool = AtomicBool::new(false);

/// The Control Manager is parked in `ct_msgup`'s `while (button & 0x0001)
/// dsptch();` (`aes/gemctrl.c:100-101`): a menu selection has been posted and
/// the press that made it is still held. Until that press is released the
/// Control Manager neither loops (`w_setactive`) nor looks at the button again
/// — so the held button cannot be taken for a fresh press on a window frame,
/// nor re-open the bar. Raised by [`ct_msgup_hold`] (from the menu
/// interception, right after `MN_SELECTED` is posted), lowered by
/// [`ct_msgup_busy`] the first time it sees the button up.
static CT_MSGUP: AtomicBool = AtomicBool::new(false);

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
/// The `!gl_ctmown` bracket is load-bearing: this handler runs from
/// [`menu::track`](super::menu::track)'s own `pump_input` while the drop-down
/// is being tracked, so the press that selects an item lands *here*, in the
/// middle of the track. With [`GL_CTMOWN`] raised the owner stays `ctl_pd`, and
/// the Event Manager's quick check for the application (`rlr == gl_mowner`,
/// `aes/gemevlib.c:185`) does not see that press — the H02 leak of the menu
/// click as `MU_BUTTON` (`TASK/H02-STATUS-DONE.MD` §4).
pub(crate) fn b_click(new: u8) {
    // "ignore it unless it represents a change" (aes/geminput.c:113)
    let prev = BUTTON.swap(new, Ordering::Relaxed);
    if new == prev {
        return;
    }
    // `if (!gl_ctmown && (new == MB_DOWN) && (button == 0)) gl_mowner = chkown();`
    // (aes/geminput.c:334-335)
    if !GL_CTMOWN.load(Ordering::Relaxed) && new == MB_DOWN && prev == 0 {
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
/// 2. The pointer entering the Control Manager's wait rectangle with the
///    buttons up hands the mouse to the Control Manager (`:387-389`), which is
///    how a hover menu takes over without a click. That rectangle is
///    `gl_ctwait` = the registered bar's `THEACTIVE` extent
///    (`aes/gemmnlib.c:717`; [`menu::in_ctwait`](super::menu::in_ctwait)) —
///    the titles' width, **not** the whole bar row: a pointer in the row right
///    of the last title stays with the application (A87). The same rectangle
///    is what [`event::menu_intercept`](super::event) tracks.
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

    // `if ((gl_mowner != ctl_pd) && (button == 0) && gl_mntree &&
    // in_mrect(&gl_ctwait)) gl_mowner = ctl_pd;` (aes/geminput.c:388-389)
    if GL_MOWNER_APP.load(Ordering::Relaxed)
        && BUTTON.load(Ordering::Relaxed) == 0
        && !super::menu::active().is_null()
        && super::menu::in_ctwait(rx, ry)
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

/// The Control Manager's `w_setactive()` (`aes/gemctrl.c:442`): give the mouse
/// to the owner of the top window — `ct_chgown(ppd, &d)` → `set_mown`
/// (`aes/gemwmlib.c:326-333`, `aes/gemctrl.c:392-397`). `ct_chgown` skips the
/// hand-over while the Control Manager holds the mouse for a menu track
/// (`if (!gl_ctmown)`, `:395`), and `set_mown` changes nothing while a button
/// is held (`if (!button)`, `aes/geminput.c:222`), so a press latched to the
/// Control Manager stays with it until the release. At single-app the top
/// window's owner is always this application.
pub(super) fn w_setactive() {
    if !GL_CTMOWN.load(Ordering::Relaxed) && BUTTON.load(Ordering::Relaxed) == 0 {
        GL_MOWNER_APP.store(true, Ordering::Relaxed);
    }
}

/// EmuTOS `ct_mouse(grabit)` (`aes/gemctrl.c:400-417`), the part that matters
/// to ownership: raise (`grab = true`) or lower [`GL_CTMOWN`]. `mn_do` brackets
/// its whole tracking loop with it (`aes/gemmnlib.c:277,513`); so does
/// [`menu::track`](super::menu::track). The mouse-form bookkeeping EmuTOS does
/// alongside (`set_mouse_to_arrow`, `gl_tmpmoff`) is the VDI cursor's business
/// here and not reproduced.
pub(super) fn ct_mouse(grab: bool) {
    GL_CTMOWN.store(grab, Ordering::Relaxed);
}

/// Enter `ct_msgup`'s wait for the selecting press to be released
/// (`while (button & 0x0001) dsptch();`, `aes/gemctrl.c:100-101`): the Control
/// Manager has posted `MN_SELECTED` and stays out of the way until the button is
/// up. Our Control Manager runs inline in the Event Manager's poll loop, so the
/// wait is a flag the loop consults ([`ct_msgup_busy`]) rather than a spin —
/// the application gets its message at once, exactly as EmuTOS's `dsptch()`
/// inside the spin lets it run.
pub(super) fn ct_msgup_hold() {
    CT_MSGUP.store(true, Ordering::Relaxed);
}

/// Is the Control Manager still inside `ct_msgup`'s wait? `true` while the
/// press that selected a menu item is held; the first call that finds the
/// button up ends the wait (`ct_msgup` returns, `ctlmgr` loops) and answers
/// `false`.
pub(super) fn ct_msgup_busy() -> bool {
    if !CT_MSGUP.load(Ordering::Relaxed) {
        return false;
    }
    if BUTTON.load(Ordering::Relaxed) & MB_DOWN != 0 {
        return true;
    }
    CT_MSGUP.store(false, Ordering::Relaxed);
    false
}

/// `ct_msgup`'s HOTCLOSE exit (`aes/gemctrl.c:86-96`): `WM_CLOSED` has been
/// posted for a window whose kind carries `HOTCLOSE`, and instead of parking
/// in the `while (button & 0x0001)` wait the Control Manager **fakes the
/// button up** — `button = 0; return;` — "otherwise a slow click will cause
/// us to stay in ctlmgr(), eating keys". From here on the AES's `button` says
/// *up* while the raw byte still says *down*: `ctlmgr` is not re-triggered
/// by the level it has already acted on, `set_mown` hands the mouse back to
/// the top window's owner at once ([`w_setactive`]), and the application's
/// `ev_multi` quick check / `abutton` (`aes/gemevlib.c:185-200`,
/// `aes/geminput.c:549`) see no press. The physical release is then a
/// non-change for [`b_click`] (as it is for `bchange`, which finds `button`
/// already 0); the next press re-synchronises everything.
pub(super) fn ct_msgup_hotclose() {
    BUTTON.store(0, Ordering::Relaxed);
}

/// EmuTOS `button` — the button state the AES has acted on
/// (`aes/geminput.c:49`); the level `ev_multi`'s quick check, `abutton` and
/// the Control Manager's wait test read (`aes/gemevlib.c:185-200`,
/// `aes/geminput.c:549`, `aes/gemctrl.c:100`). Equal to the raw VDI byte
/// (`crate::vdi::mouse_buttons`) except after [`ct_msgup_hotclose`].
pub(super) fn button() -> u8 {
    BUTTON.load(Ordering::Relaxed)
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
