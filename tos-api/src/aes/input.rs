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
//! through that difference — which process may act on the sampled level, how
//! long a click window stays open, and the button state an event *reports* —
//! is exactly what this module keeps.
//!
//! **The state at the event (`mtrans`/`pr_*`).** `bchange` records, for every
//! button transition it publishes, the state *before* it — `pr_button`,
//! `pr_mclick`, `pr_xrat`/`pr_yrat` — and counts the transitions in `mtrans`
//! (`aes/geminput.c:333-345`); `ev_rets` reports that recorded state whenever
//! more than one transition has happened since the last event return, and the
//! live state otherwise, then clears the count (`aes/gemevlib.c:42-58`). A
//! click whose release lands inside the double-click window is published as
//! two transitions at once when the window closes (`b_delay`,
//! `aes/geminput.c:155-171`), so its event reports the button **down**. That
//! is [`bchange`], [`ev_rets`] and the window close [`b_expire`] here; see
//! `docs/api/aes/event.md` "The button state an event reports".
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
/// and both ways it closes — the countdown and the forced expiry `mchange`
/// performs with `b_delay(gl_bdely)` — are [`b_expire`] clearing it.
static GL_BDELY: AtomicBool = AtomicBool::new(false);

/// EmuTOS `gl_btrue` — the true (physical) button state as `b_click` last saw
/// it (`aes/geminput.c:144-145`). Equal to [`BUTTON`] except while a
/// button-delay window is open (the changes inside it are not published until
/// it closes) and after [`ct_msgup_hotclose`] faked the AES's `button` up.
static GL_BTRUE: AtomicU8 = AtomicU8::new(0);

/// EmuTOS `gl_bdesired` — the state the open delay window counts entries into
/// (`aes/geminput.c:137`): the press that opened it.
static GL_BDESIRED: AtomicU8 = AtomicU8::new(0);

/// EmuTOS `gl_bclick` — the number of times the window's desired state was
/// entered while it was open, the opening press included
/// (`aes/geminput.c:121-122,136`).
static GL_BCLICK: AtomicI16 = AtomicI16::new(0);

/// EmuTOS `mclick` — the click count of the last published transition
/// (`aes/geminput.c:343`).
static MCLICK: AtomicI16 = AtomicI16::new(0);

/// EmuTOS `mtrans` — button transitions published since the last event return
/// (`bchange`'s `mtrans++`, `aes/geminput.c:337`; `ev_rets`'s `mtrans = 0`,
/// `aes/gemevlib.c:57`). Only "more than one" is observable: it selects the
/// recorded `pr_*` state over the live one.
static MTRANS: AtomicI16 = AtomicI16::new(0);

/// EmuTOS `pr_button`/`pr_mclick`/`pr_xrat`/`pr_yrat` — the AES's button,
/// click count and pointer position as they stood just before the last
/// published transition (`aes/geminput.c:338-341`).
static PR_BUTTON: AtomicU8 = AtomicU8::new(0);
static PR_MCLICK: AtomicI16 = AtomicI16::new(0);
static PR_XRAT: AtomicI16 = AtomicI16::new(0);
static PR_YRAT: AtomicI16 = AtomicI16::new(0);

/// The input layer's share of an AES restart (A106, [`super::reinit`]).
///
/// Two EmuTOS steps, in `gem_main`/`run_accs_and_desktop` order:
///
/// * **the button variables** (`geminit.c:661-664`): `gl_btrue = gl_bdesired
///   = gl_bdely = gl_bclick = 0` — here [`BUTTON`] and [`GL_BDELY`], plus
///   [`XRAT`]/[`YRAT`], the last reported pointer position `mchange`
///   measures against (`geminput.c:362`), which is part of the same
///   power-up-zero set (`theglo`-adjacent BSS, `gemstart.S:101-110`).
/// * **the mouse owner** (`geminit.c:888`): `gl_mowner = ctl_pd =
///   iprocess("SCRENMGR", ctlmgr)`, whose first Control-Manager pass hands
///   the mouse to the application before anything can observe it
///   (`gemctrl.c:441`) — which is why [`GL_MOWNER_APP`]'s power-up value is
///   `true`, and why that is what a restart puts back.
///
/// [`GL_CTMOWN`] and [`CT_MSGUP`] are the Control Manager's two track locks
/// (`gemctrl.c:50`, `:100-101`); a session that ended mid-track must not
/// leave either standing for the next one.
///
/// The window's `gl_btrue`/`gl_bdesired`/`gl_bclick` are the same
/// `geminit.c:661-664` set; `mclick`, `mtrans` and the `pr_*` record go back
/// to their power-up zero with `xrat`/`yrat`, so no transition of the ended
/// session is reported by the next one's first event.
pub(super) fn reinit() {
    BUTTON.store(0, Ordering::Relaxed);
    GL_BDELY.store(false, Ordering::Relaxed);
    GL_BTRUE.store(0, Ordering::Relaxed);
    GL_BDESIRED.store(0, Ordering::Relaxed);
    GL_BCLICK.store(0, Ordering::Relaxed);
    MCLICK.store(0, Ordering::Relaxed);
    MTRANS.store(0, Ordering::Relaxed);
    PR_BUTTON.store(0, Ordering::Relaxed);
    PR_MCLICK.store(0, Ordering::Relaxed);
    PR_XRAT.store(0, Ordering::Relaxed);
    PR_YRAT.store(0, Ordering::Relaxed);
    XRAT.store(0, Ordering::Relaxed);
    YRAT.store(0, Ordering::Relaxed);
    GL_MOWNER_APP.store(true, Ordering::Relaxed);
    GL_CTMOWN.store(false, Ordering::Relaxed);
    CT_MSGUP.store(false, Ordering::Relaxed);
}

// ── the two VDI mouse-interrupt vectors ────────────────────────────────────

/// `user_but` — the AES's button-change handler, EmuTOS `b_click`
/// (`aes/geminput.c:110-147`).
///
/// `new` is the new button bitmask in VDI convention (bit 0 = left), the `d0`
/// the VDI passes to `user_but` (`vdi/vdi_asm.S:270-284`). Like `b_click` this
/// ignores anything that is not a change of the **true** state `gl_btrue`
/// (`:113`). While a button-delay window is open the change is only counted —
/// an entry into the window's desired state increments `gl_bclick`
/// (`:117-123`) — and published when the window closes ([`b_expire`]);
/// otherwise it is published at once, `forkq(bchange, MAKE_ULONG(state, 1))`
/// (`:141-142`), here a direct [`bchange`] call.
///
/// EmuTOS opens the window *in* `b_click`, on the press itself, when a waiter
/// asks for more than one click (`if (gl_bpend && state)`, `:133-140`), and
/// publishes that press only when the window closes. Our Event Manager has
/// seen the press by the time it opens the window ([`b_arm`] from
/// `event::count_clicks`), so here the press is published at once with a
/// count of 1 and the window's close only corrects that count and publishes
/// the true state if it differs — the same two `bchange`s, in the same order,
/// that EmuTOS's `b_delay` forks (`:160-170`).
pub(crate) fn b_click(new: u8) {
    // "ignore it unless it represents a change" (aes/geminput.c:112-113)
    if new == GL_BTRUE.load(Ordering::Relaxed) {
        return;
    }
    if GL_BDELY.load(Ordering::Relaxed) {
        // "if the change is into the desired state, increment cnt"
        // (aes/geminput.c:119-123). The `gl_bdely += 3` extension has no
        // counterpart: `event::count_clicks` gives each further click its own
        // full window.
        if new == GL_BDESIRED.load(Ordering::Relaxed) {
            GL_BCLICK.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        let (x, y) = crate::vdi::mouse_position();
        bchange(new, 1, x, y);
    }
    // "update true state of the mouse" (aes/geminput.c:144-145)
    GL_BTRUE.store(new, Ordering::Relaxed);
}

/// EmuTOS `bchange` (`aes/geminput.c:329-345`) — publish one button
/// transition: `new` becomes the AES's `button`, with `clicks` as its click
/// count, and the state it replaces is recorded for [`ev_rets`]. `(x, y)` is
/// the AES's pointer position at that moment (`xrat`/`yrat`); the VDI cursor
/// position is used, which the packet carrying the change has not moved yet
/// (`user_but` runs before the position is touched, `vdi/vdi_asm.S:261-284`).
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
fn bchange(new: u8, clicks: i16, x: i16, y: i16) {
    let button = BUTTON.load(Ordering::Relaxed);
    // `if (!gl_ctmown && (new == MB_DOWN) && (button == 0)) gl_mowner = chkown();`
    // (aes/geminput.c:334-335)
    if !GL_CTMOWN.load(Ordering::Relaxed) && new == MB_DOWN && button == 0 {
        // chkown: +1 → gl_cowner, 0 → D.w_win[0].w_owner (both the app at
        // single-app), −1 → ctl_pd.
        GL_MOWNER_APP.store(super::window::chk_ctrl(x, y) >= 0, Ordering::Relaxed);
    }
    // `mtrans++; pr_button = button; pr_mclick = mclick; pr_xrat = xrat;
    // pr_yrat = yrat; button = new; mclick = clicks;` (aes/geminput.c:337-343).
    // Saturating: only "> 1" is ever read, and a C `WORD` overflow is no
    // contract.
    let _ = MTRANS.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |m| Some(m.saturating_add(1)));
    PR_BUTTON.store(button, Ordering::Relaxed);
    PR_MCLICK.store(MCLICK.load(Ordering::Relaxed), Ordering::Relaxed);
    PR_XRAT.store(x, Ordering::Relaxed);
    PR_YRAT.store(y, Ordering::Relaxed);
    BUTTON.store(new, Ordering::Relaxed);
    MCLICK.store(clicks, Ordering::Relaxed);
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
    // `b_delay(gl_bdely)` forks the window's `bchange`s; they run after this
    // `mchange` has finished (the fork ring is FIFO), so they see the new
    // `xrat`/`yrat` — hence the expiry is applied at the end, below.
    let expire = bdely_pending() && (dx > 2 || dx < -2 || dy > 2 || dy < -2);

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

    if expire {
        b_expire(rx, ry);
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
/// (`aes/gemwmlib.c:326-333`, `aes/gemctrl.c:392-397`). At single-app the top
/// window's owner is always this application; the two guards are
/// [`ct_chgown`]'s and [`set_mown`]'s, so a press latched to the Control
/// Manager stays with it until the release.
pub(super) fn w_setactive() {
    ct_chgown(true);
}

/// EmuTOS `get_mown(&pmown)` (`aes/geminput.c:209-212`) — the current mouse
/// owner, as [`own_mouse`] reports it: `true` = the application (`rlr`),
/// `false` = `ctl_pd`. `fm_own` saves it on entry (`ml_pmown`,
/// `aes/gemfmlib.c:92`) to hand the mouse back to that owner on exit.
pub(super) fn get_mown() -> bool {
    GL_MOWNER_APP.load(Ordering::Relaxed)
}

/// EmuTOS `ct_chgown(mpd, pr)` (`aes/gemctrl.c:392-397`): `set_ctrl(pr); if
/// (!gl_ctmown) set_mown(mpd);` — the mouse changes hands only while no menu
/// track holds it. The control rectangle half is not stored: EmuTOS's `ctrl`
/// has exactly two sources, and [`window::chk_ctrl`](super::window::chk_ctrl)
/// reads both live (`ml_ocnt > 0` → the whole screen; else the top window's
/// work area). `app` names the owner as [`get_mown`] does.
pub(super) fn ct_chgown(app: bool) {
    if !GL_CTMOWN.load(Ordering::Relaxed) {
        set_mown(app);
    }
}

/// EmuTOS `set_mown(mp)` (`aes/geminput.c:220-235`): `if (!button) gl_cowner =
/// gl_mowner = mp;` — "the mouse should only be transferred with the buttons
/// in an up state" (`:216-217`); with a button held nothing changes. The
/// `post_mouse`/`post_button` that follow in EmuTOS (`:231-233`) wake the new
/// owner's parked waits; our Event Manager polls the live position and button
/// against `own_mouse()` on every pass, so the hand-over itself is the wake.
fn set_mown(app: bool) {
    if BUTTON.load(Ordering::Relaxed) == 0 {
        GL_MOWNER_APP.store(app, Ordering::Relaxed);
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
/// change of the true state `gl_btrue` for [`b_click`], which publishes it,
/// and `bchange` finds `button` already 0 — the release is counted in
/// `mtrans` with `pr_button = 0`, nothing else moves; the next press
/// re-synchronises everything.
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

/// Open the button-delay window — EmuTOS `gl_bclick = 1; gl_bdesired =
/// state; gl_bdely = gl_dclick;` in `b_click` (`aes/geminput.c:135-140`).
/// EmuTOS opens it only when someone is waiting for more than one click *and*
/// the change is a press (`if (gl_bpend && state)`, `:133`); the caller
/// applies both conditions. The desired state is the true state the press
/// left, which [`b_click`] has already published (see there).
pub(super) fn b_arm() {
    GL_BDESIRED.store(GL_BTRUE.load(Ordering::Relaxed), Ordering::Relaxed);
    GL_BCLICK.store(1, Ordering::Relaxed);
    GL_BDELY.store(true, Ordering::Relaxed);
}

/// Close the button-delay window normally — the `gl_bdely` countdown reaching
/// zero (`b_delay`, `aes/geminput.c:155-171`), here the counting loop finishing.
/// A window that the pointer already closed ([`mchange`]) is not closed twice.
pub(super) fn b_disarm() {
    let (x, y) = crate::vdi::mouse_position();
    b_expire(x, y);
}

/// Is a button-delay window open? EmuTOS `if (gl_bdely)`
/// (`aes/geminput.c:117,366`).
pub(super) fn bdely_pending() -> bool {
    GL_BDELY.load(Ordering::Relaxed)
}

/// The delay window closing — `b_delay` once `gl_bdely` reaches zero
/// (`aes/geminput.c:155-171`), whether by the countdown ([`b_disarm`]) or by
/// `mchange`'s `b_delay(gl_bdely)` (`:365-368`). EmuTOS then publishes
/// `bchange(gl_bdesired, gl_bclick)` and, if the button no longer is in that
/// state, `bchange(gl_btrue, 1)` (`:160-170`). The first of the two has
/// already been published here with a count of 1 when the window opened
/// ([`b_click`]), so it only gets its real count; the second is published as
/// in EmuTOS. A click released inside the window therefore leaves two
/// transitions behind — `mtrans > 1` with `pr_button` = the press — which is
/// what [`ev_rets`] reports. `(x, y)` is the pointer position at the close.
fn b_expire(x: i16, y: i16) {
    if !GL_BDELY.swap(false, Ordering::Relaxed) {
        return;
    }
    MCLICK.store(GL_BCLICK.load(Ordering::Relaxed), Ordering::Relaxed);
    let btrue = GL_BTRUE.load(Ordering::Relaxed);
    if btrue != GL_BDESIRED.load(Ordering::Relaxed) {
        bchange(btrue, 1, x, y);
    }
}

/// EmuTOS `ev_rets` (`aes/gemevlib.c:42-58`), the mouse half: the pointer
/// position and button state an event reports — the state recorded before the
/// last published transition (`pr_xrat`/`pr_yrat`/`pr_button`) when more than
/// one transition has been published since the last event return
/// (`mtrans > 1`), else the live `xrat`/`yrat`/`button` — then `mtrans = 0`.
///
/// Every AES-internal wait whose EmuTOS counterpart is an `ev_multi`/
/// `ev_button` (the Form Manager's `fm_do`, `gr_stilldn`, `mn_do`, the
/// pop-up's `handle_popup`, the Control Manager's own loop) consumes the
/// transitions it returns on in EmuTOS; those call this too and drop the
/// result, so a click the AES itself acted on is not reported again by the
/// application's next event. The live position is the VDI cursor's, as
/// everywhere else in the Event Manager.
pub(super) fn ev_rets() -> (i16, i16, u8) {
    let r = if MTRANS.load(Ordering::Relaxed) > 1 {
        (
            PR_XRAT.load(Ordering::Relaxed),
            PR_YRAT.load(Ordering::Relaxed),
            PR_BUTTON.load(Ordering::Relaxed),
        )
    } else {
        let (x, y) = crate::vdi::mouse_position();
        (x, y, BUTTON.load(Ordering::Relaxed))
    };
    MTRANS.store(0, Ordering::Relaxed);
    r
}

/// `ev_multi`'s quick check on the recorded state, `if ((mtrans > 1) &&
/// downorup(pr_button, buparm)) { … prets[5] = pr_mclick; }`
/// (`aes/gemevlib.c:190-194`): `Some((pr_button, pr_mclick))` while more than
/// one transition is pending, else `None`. Read-only — the count is cleared
/// by the [`ev_rets`] that ends the call.
pub(super) fn pending_transition() -> Option<(u8, i16)> {
    (MTRANS.load(Ordering::Relaxed) > 1)
        .then(|| (PR_BUTTON.load(Ordering::Relaxed), PR_MCLICK.load(Ordering::Relaxed)))
}

// ── the mouse wheel (CONF_WITH_EXTENDED_MOUSE) ─────────────────────────────

/// `WM_ARROWED` (`include/aesdefs.h:233`).
const WM_ARROWED: i16 = 24;
/// `WM_ARROWED` sub-action: scroll one line up (`include/aesdefs.h:248`).
const WA_UPLINE: i16 = 2;
/// `WM_ARROWED` sub-action: scroll one line down (`include/aesdefs.h:249`).
const WA_DNLINE: i16 = 3;

/// EmuTOS `wheel_change(wheel_number, wheel_amount)` (`aes/geminput.c:394-
/// 420`) — the AES's end of the wheel chain, reached through the VDI's
/// `user_wheel` vector ([`crate::vdi::mouse::install_aes_wheel`],
/// [`super::aes_wheel`]).
///
/// EmuTOS's body, complete:
///
/// ```c
/// /* Ignore the wheel messages if the menu is active */
/// if (gl_mowner == ctl_pd)
///     return;
/// if (wheel_amount > 0)       type = WA_DNLINE;
/// else if (wheel_amount < 0)  type = WA_UPLINE;
/// else                        return;
/// /* We have a problem here.
///  * This function is called by forker(), where rlr is set to -1.
///  * As a result, we can't call ap_sendmsg() which internally calls iasync().
///  * Uncomment the following code when some solution has been found.
///  */
/// /*
///     assert(rlr != (AESPD *)-1);
///     wh = wm_find(xrat, yrat);
///     ap_sendmsg(appl_msg, WM_ARROWED, D.w_win[wh].w_owner, wh, type, 0,0,0);
/// */
/// ```
///
/// ## The named deviation: NewTOS sends the message
///
/// **What EmuTOS does here.** It computes the action and then throws the
/// event away. Its own comment names the reason: `wheel_change` runs from
/// `forker()`, which parks the running-process pointer at a sentinel —
/// `oldrl = rlr; rlr = (AESPD *) -1;` (`aes/gemdisp.c:105-106`) — for the
/// whole fork-ring drain. `ap_sendmsg` dereferences exactly that pointer
/// (`ap_msg[1] = rlr->p_pid`, `aes/gemwmlib.c:855`) and then enters
/// `ap_rdwr`/`iasync`, the asynchronous send that reschedules. Neither is
/// legal against `rlr == -1`, so the three lines stay commented out and a
/// mouse wheel on EmuTOS does nothing.
///
/// **Why we go further.** The user's decision of 2026-09-25 is that the wheel
/// must scroll (`TASK/GUI04.md`). This is not the "rationalised EmuTOS
/// deviation" the project forbids: EmuTOS states in its own source that a
/// solution is *missing*, not that the behaviour is intended.
///
/// **How, and why EmuTOS's obstacle is not ours.** NewTOS has no fork ring
/// and no `forker()`. A wheel detent travels `Host::pump_input` →
/// `usb::poll` → `ikbd::feed` → `mousexvec` → `call_user_wheel` → here, and
/// `pump_input` is only ever called **synchronously** from EL1 code running
/// on behalf of a well-defined caller: an AES/GEMDOS trap of the running EL0
/// process, or the kernel desktop's own loop. There is no point at which a
/// "no running process" sentinel is installed, so the precondition of
/// EmuTOS's comment simply does not arise. Its two concrete obstacles are
/// gone with it:
///
/// * `rlr->p_pid` — our message poster takes `msg[1]` as an explicit
///   argument. `wheel_change` is control-manager-side code (EmuTOS's
///   `aes_wheel` is forked exactly like `far_mcha`/`mchange`), so it stamps
///   [`super::appl::SCRENMGR_PID`], the same stamp
///   [`window::intercept`](super::window::intercept) uses for every `WM_*` it
///   posts (spec § "AES process identity and pid", contract (b)).
/// * `iasync` — [`pd::post_msg`](super::pd::post_msg) enqueues into the
///   owner's FIFO and flags a sleeping waiter through `sched::wake`; it does
///   not dispatch and cannot re-enter the caller.
///
/// So the delivery is the ordinary one: the same `ap_sendmsg` → `aqueue` →
/// `doq` routing that `send_redraw` and the frame interception already use,
/// including `doq`'s `WM_ARROWED` coalescing (`aes/gemqueue.c:63-67`) — a
/// burst of detents the application has not read yet collapses to the newest,
/// exactly as a burst of scroll-arrow clicks does.
///
/// The menu guard is EmuTOS's, unchanged: while the Control Manager owns the
/// mouse (`gl_mowner == ctl_pd`, i.e. a menu is being tracked) the wheel is
/// ignored.
pub(crate) fn wheel_change(_wheel_number: i16, wheel_amount: i16) {
    // `if (gl_mowner == ctl_pd) return;` (`aes/geminput.c:400-402`)
    if !GL_MOWNER_APP.load(Ordering::Relaxed) {
        return;
    }
    // `if (wheel_amount > 0) WA_DNLINE; else if (< 0) WA_UPLINE; else return;`
    // (`:404-409`)
    let ty = match wheel_amount {
        a if a > 0 => WA_DNLINE,
        a if a < 0 => WA_UPLINE,
        _ => return,
    };
    // `wh = wm_find(xrat, yrat)` (`:418`) — the window under the **last
    // reported** pointer position, the same `xrat`/`yrat` `mchange` maintains
    // (`:370-371`), not a fresh sample.
    let wh = super::window::wm_find(XRAT.load(Ordering::Relaxed), YRAT.load(Ordering::Relaxed));
    let Some(owner) = super::window::owner_of_window(wh) else {
        return;
    };
    // `ap_sendmsg(appl_msg, WM_ARROWED, D.w_win[wh].w_owner, wh, type, 0,0,0)`
    // (`:419`): msg[0] type, msg[1] sender pid, msg[2] 0, msg[3] the window
    // handle, msg[4] the action, msg[5..7] 0 (`aes/gemwmlib.c:851-862`).
    let mut msg = [0i16; 8];
    msg[0] = WM_ARROWED;
    msg[1] = super::appl::SCRENMGR_PID;
    msg[3] = wh;
    msg[4] = ty;
    // SAFETY: single trap path (see the module note); `post_msg` hands the
    // message to the live descriptor of `owner`.
    unsafe {
        super::pd::post_msg(owner, msg);
    }
}
