//! GUI04 — `wheel_change`: the AES end of the mouse wheel.
//!
//! EmuTOS `wheel_change` (`references/emutos/aes/geminput.c:394-420`) computes
//! the `WM_ARROWED` action and then throws the event away, because it runs
//! from `forker()` with `rlr == (AESPD *) -1` (`aes/gemdisp.c:105-106`) and
//! `ap_sendmsg` would dereference that sentinel. NewTOS has no fork ring — the
//! wheel arrives inside the caller's own trap through `Host::pump_input` — so
//! the message is really sent; the reasoning is written out at
//! [`tos_api::aes::aes_wheel`]'s callee in `aes/input.rs`.
//!
//! What is measured here is exactly the three lines EmuTOS left commented out:
//!
//! ```c
//! wh = wm_find(xrat, yrat);
//! ap_sendmsg(appl_msg, WM_ARROWED, D.w_win[wh].w_owner, wh, type, 0, 0, 0);
//! ```
//!
//! plus the two guards above them: `wheel_amount == 0` returns without a
//! message, a negative amount is `WA_UPLINE` and a positive one `WA_DNLINE`
//! (`:404-409`).
//!
//! The menu guard (`if (gl_mowner == ctl_pd) return;`, `:400-402`) is not
//! reachable from here — it needs a registered menu bar and a pointer sweep
//! through its title row — and is measured on the target instead
//! (`measurement-2026-09-26-gui04`, run C).

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{Pb, AES_LOCK};

// AES opcodes.
const WIND_CREATE: i16 = 100;
const WIND_OPEN: i16 = 101;
const WIND_CLOSE: i16 = 102;
const WIND_DELETE: i16 = 103;

// Message words (`include/aesdefs.h:233,248-249`).
const WM_ARROWED: i16 = 24;
const WA_UPLINE: i16 = 2;
const WA_DNLINE: i16 = 3;
/// The Screen Manager's sender stamp — `ap_sendmsg`'s `rlr->p_pid` for a
/// control-manager-side poster (`aes/geminit.c:888`).
const SCRENMGR_PID: i16 = 1;

/// The window this test scrolls: a rectangle well away from the menu bar, so
/// the pointer can sit inside it without handing the mouse to the Control
/// Manager.
const WX: i16 = 100;
const WY: i16 = 120;
const WW: i16 = 240;
const WH: i16 = 160;

struct Win {
    handle: i16,
}

impl Win {
    /// `wind_create(0, WX, WY, WW, WH)` + `wind_open` — the smallest window an
    /// application can have under the pointer.
    fn open(pb: &mut Pb) -> Win {
        pb.poison();
        pb.intin[0] = 0; // no gadgets: nothing to intercept, only a rectangle
        pb.intin[1] = WX;
        pb.intin[2] = WY;
        pb.intin[3] = WW;
        pb.intin[4] = WH;
        pb.call(WIND_CREATE, 5, 1, 0, 0);
        let handle = pb.intout[0];
        assert!(handle > 0, "wind_create returned no handle ({handle})");

        pb.poison();
        pb.intin[0] = handle;
        pb.intin[1] = WX;
        pb.intin[2] = WY;
        pb.intin[3] = WW;
        pb.intin[4] = WH;
        pb.call(WIND_OPEN, 5, 1, 0, 0);
        Win { handle }
    }

    fn close(self, pb: &mut Pb) {
        pb.poison();
        pb.intin[0] = self.handle;
        pb.call(WIND_CLOSE, 1, 1, 0, 0);
        pb.poison();
        pb.intin[0] = self.handle;
        pb.call(WIND_DELETE, 1, 1, 0, 0);
    }
}

/// Put the pointer at `(x, y)` through the real input path, so the AES's
/// `xrat`/`yrat` — what `wheel_change` reads — follow it. `mchange` only runs
/// for a packet that carried movement, so the delta is computed from the live
/// cursor position.
fn move_pointer_to(x: i16, y: i16) {
    let backend = tos_api::host::host().unwrap().raster().unwrap();
    let (cx, cy) = tos_api::vdi::mouse_position();
    let (dx, dy) = (x - cx, y - cy);
    if dx == 0 && dy == 0 {
        // A zero packet would not reach `mchange`; nudge and come back.
        tos_api::vdi::feed_relative(1, 1, 0, backend);
        tos_api::vdi::feed_relative(-1, -1, 0, backend);
        return;
    }
    tos_api::vdi::feed_relative(dx, dy, 0, backend);
    assert_eq!(tos_api::vdi::mouse_position(), (x, y), "pointer parked at {x},{y}");
}

/// The message at the front of the application's queue, if any.
fn pending() -> Option<[i16; 8]> {
    tos_api::aes::window::test_pending_msg(0)
}

fn clear() {
    tos_api::aes::window::test_clear_msg(0);
}

#[test]
fn a_wheel_detent_posts_wm_arrowed_to_the_window_under_the_pointer() {
    let _guard = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    harness::context();
    let mut pb = Pb::new();
    let win = Win::open(&mut pb);

    // The pointer inside the window, and the redraw `wind_open` posted out of
    // the way, so the only message left is the wheel's.
    move_pointer_to(WX + WW / 2, WY + WH / 2);
    clear();

    // `wheel_amount < 0` -> WA_UPLINE (`aes/geminput.c:407-408`).
    tos_api::aes::aes_wheel(0, -1);
    let msg = pending().expect("a wheel detent must post a message");
    assert_eq!(msg[0], WM_ARROWED, "message type");
    assert_eq!(msg[1], SCRENMGR_PID, "sender stamp: the control-manager side");
    assert_eq!(msg[2], 0, "ap_sendmsg writes 0 into msg[2]");
    assert_eq!(msg[3], win.handle, "the window under the pointer");
    assert_eq!(msg[4], WA_UPLINE, "a negative amount scrolls up");
    assert_eq!(&msg[5..8], &[0, 0, 0], "the remaining words are 0");
    clear();

    // `wheel_amount > 0` -> WA_DNLINE (`:405-406`).
    tos_api::aes::aes_wheel(0, 1);
    let msg = pending().expect("a wheel detent must post a message");
    assert_eq!(msg[0], WM_ARROWED);
    assert_eq!(msg[3], win.handle);
    assert_eq!(msg[4], WA_DNLINE, "a positive amount scrolls down");
    clear();

    // `wheel_amount == 0` -> `return` without a message (`:409`).
    tos_api::aes::aes_wheel(0, 0);
    assert!(pending().is_none(), "a zero amount posts nothing");

    // The horizontal wheel takes the same road: `wheel_change` never looks at
    // `wheel_number`, so number 1 posts the same WA_UPLINE/WA_DNLINE — which is
    // EmuTOS' behaviour, not an omission (`:404-409` reads only the amount).
    tos_api::aes::aes_wheel(1, -1);
    let msg = pending().expect("the horizontal wheel posts too");
    assert_eq!(msg[4], WA_UPLINE);
    clear();

    win.close(&mut pb);
}

#[test]
fn a_wheel_detent_over_no_window_posts_nothing() {
    let _guard = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    harness::context();

    // No application window is open, so `wm_find` finds at most the desktop
    // window, whose slot has no owner in this fixture — `D.w_win[wh].w_owner`
    // resolves to nothing and there is no queue to post into.
    move_pointer_to(10, 400);
    clear();
    tos_api::aes::aes_wheel(0, -1);
    assert!(pending().is_none(), "no window under the pointer, no message");
}
