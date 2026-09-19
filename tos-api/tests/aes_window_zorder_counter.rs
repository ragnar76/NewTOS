//! The stacking counter behind `wind_open` / `wind_set(WF_TOP)` survives its
//! 32768th step — `TASK/A52.md`.
//!
//! EmuTOS keeps **no** counter: its z-order *is* the sibling chain of the
//! window tree. `wm_open` appends the window at `W_TREE[ROOT].ob_tail`
//! (`w_obadd`, `references/emutos/aes/gemwmlib.c:186-201`, from `:1425`),
//! `wm_mktop` moves a topped window there with `ob_order(gl_wtree, w_handle,
//! NIL)` (`:1609`), and "the top window" is `W_TREE[ROOT].ob_tail`
//! (`:1022,1093`). Nothing in that can overflow. NewTOS orders its slots by a
//! monotonically increasing `i16` instead, so the counter is an
//! implementation detail with one obligation: it must never stop answering a
//! value above every open window's — and it must not abort. `z + 1` at
//! `i16::MAX` did exactly that in the dev profile (`overflow-checks = true`).
//!
//! The fix renumbers the open windows into `1..=n` in their current order the
//! moment the counter reaches `i16::MAX`, and carries on from `n + 1`.
//! Saturating would tie every later window at `i16::MAX` and the tie-break is
//! the slot index — a wrong order; wrapping to `i16::MIN` would put the next
//! window *under* every existing one. Renumbering preserves the order
//! exactly, which is the whole contract.
//!
//! The test drives the counter to `i16::MAX - 1` through a test hook, opens
//! two windows across the boundary and tops one, and checks the order the
//! only way an application can: `wind_get(0, WF_TOP)` — `w_top()`, the
//! topmost open window (`gemwmlib.c:1553-1554`, `w_top` `:315-318`) — and `WF_OWNER`'s `int_out[4]`,
//! the bottommost one (`W_TREE[ROOT].ob_head`, `:1588`).

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};
use tos_api::aes::window::{test_set_next_z, test_window_z};

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_GET: i16 = 104;
const OP_WIND_SET: i16 = 105;
const OP_WIND_NEW: i16 = 109;

const WF_TOP: i16 = 10;
const WF_OWNER: i16 = 20;

const KIND: i16 = 0x0FEF;

type R = (i16, i16, i16, i16);
const W1: R = (30, 40, 320, 240);
const W2: R = (60, 80, 200, 160);
const W3: R = (180, 160, 200, 160);

fn lock() -> std::sync::MutexGuard<'static, ()> {
    AES_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// `wind_new` — the window manager's own reset (`gemwmlib.c:1875-1893`),
/// which also restarts the counter at 1.
fn wind_new(pb: &mut Pb) {
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);
}

fn wind_create(pb: &mut Pb) -> i16 {
    pb.poison();
    pb.intin[0] = KIND;
    pb.intin[1] = 10;
    pb.intin[2] = 20;
    pb.intin[3] = 300;
    pb.intin[4] = 200;
    pb.call(OP_WIND_CREATE, 5, 1, 0, 0);
    let h = pb.intout[0];
    assert!(h > 0, "wind_create failed ({h})");
    h
}

fn wind_open(pb: &mut Pb, handle: i16, r: R) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = r.0;
    pb.intin[2] = r.1;
    pb.intin[3] = r.2;
    pb.intin[4] = r.3;
    pb.call(OP_WIND_OPEN, 5, 1, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_open({handle}, {r:?}) failed");
}

fn wind_set_top(pb: &mut Pb, handle: i16) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = WF_TOP;
    pb.intin[2] = 0;
    pb.intin[3] = 0;
    pb.intin[4] = 0;
    pb.intin[5] = 0;
    pb.call(OP_WIND_SET, 6, 1, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_set({handle}, WF_TOP) failed");
}

/// `wind_get(0, WF_TOP)` → `int_out[1]`, the topmost open window.
fn top(pb: &mut Pb) -> i16 {
    pb.poison();
    pb.intin[0] = 0;
    pb.intin[1] = WF_TOP;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1);
    pb.intout[1]
}

/// `wind_get(0, WF_OWNER)` → `int_out[4]`, the bottommost open window
/// (`W_TREE[ROOT].ob_head`, `gemwmlib.c:1588`).
fn bottom(pb: &mut Pb) -> i16 {
    pb.poison();
    pb.intin[0] = 0;
    pb.intin[1] = WF_OWNER;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1);
    pb.intout[4]
}

/// Three windows, opened 1, 2, 3 with the counter at `i16::MAX - 1` before
/// the second open: the third open is the step that used to overflow. Then
/// window 1 is topped — one more step past the edge. Order after every
/// step: bottom and top as an application sees them, and the raw values
/// strictly ascending where the order says they must be.
#[test]
fn the_counter_steps_past_i16_max_without_a_panic_and_keeps_the_order() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = [wind_create(&mut pb), wind_create(&mut pb), wind_create(&mut pb)];
    wind_open(&mut pb, h[0], W1);
    assert_eq!((bottom(&mut pb), top(&mut pb)), (h[0], h[0]));

    let before = test_set_next_z(i16::MAX - 1);
    assert!(before > 0 && before < 100, "wind_new restarted the counter ({before})");

    wind_open(&mut pb, h[1], W2); // takes i16::MAX - 1
    assert_eq!((bottom(&mut pb), top(&mut pb)), (h[0], h[1]));
    wind_open(&mut pb, h[2], W3); // the counter stands at i16::MAX: this step overflowed
    assert_eq!((bottom(&mut pb), top(&mut pb)), (h[0], h[2]));

    wind_set_top(&mut pb, h[0]); // the step past the edge
    assert_eq!((bottom(&mut pb), top(&mut pb)), (h[1], h[0]), "1 topped: order is 2, 3, 1");

    wind_set_top(&mut pb, h[1]);
    assert_eq!((bottom(&mut pb), top(&mut pb)), (h[2], h[1]), "2 topped: order is 3, 1, 2");

    let z: Vec<i16> = [h[2], h[0], h[1]].iter().map(|&w| test_window_z(w).expect("open window")).collect();
    assert!(z[0] < z[1] && z[1] < z[2], "raw stacking values must be strictly ascending bottom→top: {z:?}");

    wind_new(&mut pb);
}
