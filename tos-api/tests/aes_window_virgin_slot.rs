//! A handle that has never been in use is **not** the same as a freed one.
//!
//! `wm_new` sweeps under a `VF_INUSE` test — `if (D.w_win[wh].w_flags &
//! VF_INUSE) wm_delete(wh);` (`references/emutos/aes/gemwmlib.c:1889-1890`,
//! pinned tree `1a7484aba1bfba23e2c0899e243b6d8177fe41df`) — so a slot nobody
//! has taken yet keeps its BSS zeros, while a slot `wm_delete` has been through
//! keeps `gl_rscreen`/`gl_rfull` (`:1477-1480`). Since `wm_create` writes
//! `WS_CURR`, `WS_PREV` and `WS_FULL` but never `WS_WORK` (`:1377-1379`, and
//! `w_setup` does not touch `w_work` either, `:205-228`), the difference is
//! visible: the first window ever created in a handle answers
//! `wind_get(WF_WXYWH)` with `0,0,0,0`, a later one in the same handle with
//! `gl_rfull`.
//!
//! That is the guard against fixing A33 by filling the table with `gl_rfull`
//! instead of routing the in-use slots through `wm_delete`. Both readings
//! satisfy the measured cases 340/344 of
//! `tosdump/cases/aes-r3-wind-state.tsv` (whose `!reset wind_new` line always
//! recycles), and only one of them is what EmuTOS does.
//!
//! **This file holds exactly one test on purpose.** The window table is a
//! process-global static and an integration test file is its own process, so
//! the "never in use" precondition is only guaranteed when nothing else in the
//! same binary has created a window first. Adding a second test here would
//! silently break it.
//!
//! Ticket: `TASK/A33.md`.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_GET: i16 = 104;
const OP_WIND_NEW: i16 = 109;

const WF_WXYWH: i16 = 4;

type R = (i16, i16, i16, i16);

const KIND: i16 = 0x0FEF;
const FULL: R = (10, 20, 300, 200);

/// `gl_rfull` at this harness' geometry (`aes/gemgraf.c:424`).
const GL_RFULL: R = (0, 19, 640, 461);

fn wind_new(pb: &mut Pb) {
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);
}

fn wind_create(pb: &mut Pb, kind: i16, full: R) -> i16 {
    pb.poison();
    pb.intin[0] = kind;
    pb.intin[1] = full.0;
    pb.intin[2] = full.1;
    pb.intin[3] = full.2;
    pb.intin[4] = full.3;
    pb.call(OP_WIND_CREATE, 5, 1, 0, 0);
    let h = pb.intout[0];
    assert!(h >= 1, "wind_create({kind:#06x}, {full:?}) returned {h}");
    h
}

fn wind_get_rect(pb: &mut Pb, handle: i16, field: i16) -> R {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_get({handle}, {field}) returned FALSE");
    (pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4])
}

/// The two answers, in one run, from the same handle: virgin first, recycled
/// second. The harness `context()` runs `appl_init` / `graf_handle` /
/// `v_opnvwk` and creates no window, so the table is untouched when the first
/// `wind_create` below runs.
#[test]
fn a_handle_answers_zero_before_its_first_life_and_gl_rfull_after_it() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // No window exists yet, so this sweep frees nothing — it may not arm the
    // slot either.
    wind_new(&mut pb);

    let h = wind_create(&mut pb, KIND, FULL);
    assert_eq!(
        wind_get_rect(&mut pb, h, WF_WXYWH),
        (0, 0, 0, 0),
        "handle {h} has never been in use, so wm_new's VF_INUSE test (:1889) \
         skipped it and its WS_WORK is still the BSS zero EmuTOS starts with"
    );

    // Now it has had a life; the sweep sends it through wm_delete (:1890),
    // which writes WS_WORK = gl_rfull (:1480).
    wind_new(&mut pb);

    let again = wind_create(&mut pb, KIND, FULL);
    assert_eq!(again, h, "the freed handle is the first one free again");
    assert_eq!(
        wind_get_rect(&mut pb, again, WF_WXYWH),
        GL_RFULL,
        "same handle, same create arguments, different answer — the difference \
         is the slot's history, which is the whole point of :1480"
    );

    wind_new(&mut pb);
}
