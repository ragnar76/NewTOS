//! `wind_delete` must arm the slot it frees, not blank it.
//!
//! EmuTOS `wm_delete` (`references/emutos/aes/gemwmlib.c:1466-1485`, pinned tree
//! `1a7484aba1bfba23e2c0899e243b6d8177fe41df`) writes **four** rectangles into
//! the slot it gives back, plus the flags, the owner and the rectangle list:
//!
//! ```c
//! if (pwin->w_flags & VF_ISOPEN) wm_close(w_handle);
//! newrect(gl_wtree, w_handle);                    /* give back recs. */
//! w_setsize(WS_CURR, w_handle, &gl_rscreen);      /* :1477 */
//! w_setsize(WS_PREV, w_handle, &gl_rscreen);      /* :1478 */
//! w_setsize(WS_FULL, w_handle, &gl_rfull);        /* :1479 */
//! w_setsize(WS_WORK, w_handle, &gl_rfull);        /* :1480 */
//! pwin->w_flags = 0;                              /* :1481 */
//! pwin->w_owner = NULL;                           /* :1482 */
//! ```
//!
//! **Only one of the four is observable, and it is observable only from the
//! slot's next life.** While the slot is free `get_pwin` refuses it
//! (`:1470-1472`, `:1391-1404`), so no `wind_get` can read any of the four. The
//! next `wm_create` then overwrites `WS_CURR` and `WS_PREV` with `gl_rzero` and
//! `WS_FULL` with its own rectangle (`:1377-1379`) — but it never writes
//! `WS_WORK`, and neither does `w_setup` (`:205-228`). So `gl_rfull` is what a
//! created-but-unopened window answers `wind_get(WF_WXYWH)` with, measured:
//! `tosdump/cases/aes-r3-wind-state.tsv` cases 340 and 344 against
//! `tosdump/reference/EMUTOS14-AES-r3.BIN`, both `0,19,640,461`.
//!
//! `tos-api/tests/aes_window_create_work.rs` reaches that number through
//! `wind_new` (`wm_new` → `wm_delete`). This file reaches it through the
//! `wind_delete` **opcode**, states the three rectangles that must *not* leak
//! into the next window, and pins the free-slot rule `wm_new` has to obey:
//! a slot that is not in use is not touched at all.
//!
//! Not restated here because measured reference cases already carry them:
//! `wind_delete` of a never-created handle, of `99` and of `-1` all return FALSE
//! (`cases_r2_wind.rs` cases 195, 201, 202), and of a created resp. open window
//! TRUE (cases 197, 200).
//!
//! Ticket: `TASK/A33.md`.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_GET: i16 = 104;
const OP_WIND_DELETE: i16 = 103;
const OP_WIND_NEW: i16 = 109;

const WF_WXYWH: i16 = 4;
const WF_CXYWH: i16 = 5;
const WF_PXYWH: i16 = 6;
const WF_FXYWH: i16 = 7;

type R = (i16, i16, i16, i16);

/// The create pair `tosdump/cases/aes-r3-wind-state.tsv` uses: every gadget bit
/// except `INFO`, maximum extent `10,20,300,200`.
const KIND: i16 = 0x0FEF;
const FULL: R = (10, 20, 300, 200);
const OPEN_R: R = (30, 40, 320, 240);

/// `gl_rzero` (`aes/gemgraf.c:425`).
const GL_RZERO: R = (0, 0, 0, 0);

/// `gl_rfull` at this harness' geometry — the screen minus the menu row
/// (`r_set(&gl_rfull, 0, gl_hbox, gl_width, gl_height - gl_hbox)`,
/// `aes/gemgraf.c:424`).
const GL_RFULL: R = (0, 19, 640, 461);

/// `gl_rscreen` — the whole screen (`aes/gemgraf.c:423`). Named so the
/// leak assertions below say what they are guarding against.
const GL_RSCREEN: R = (0, 0, 640, 480);

// ── the calls ────────────────────────────────────────────────────────────

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

fn wind_delete(pb: &mut Pb, handle: i16) -> i16 {
    pb.poison();
    pb.intin[0] = handle;
    pb.call(OP_WIND_DELETE, 1, 1, 0, 0);
    pb.intout[0]
}

fn wind_get_rect(pb: &mut Pb, handle: i16, field: i16) -> R {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_get({handle}, {field}) returned FALSE");
    (pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4])
}

/// `wind_get` that is expected to be refused — the return value only.
fn wind_get_ret(pb: &mut Pb, handle: i16, field: i16) -> i16 {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    pb.intout[0]
}

// ── the tests ────────────────────────────────────────────────────────────

/// The `wind_delete` opcode arms the slot: a window created in a handle that
/// `wind_delete` freed answers `WF_WXYWH` with `gl_rfull`, because `wm_delete`
/// put it there (`:1480`) and nothing in the create path overwrites it.
#[test]
fn a_window_created_in_a_deleted_handle_inherits_gl_rfull_as_its_work_rect() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = wind_create(&mut pb, KIND, FULL);
    wind_open(&mut pb, h, OPEN_R);
    assert_eq!(wind_delete(&mut pb, h), 1, "wind_delete of an open window must return TRUE");

    let again = wind_create(&mut pb, KIND, FULL);
    assert_eq!(again, h, "the freed handle is the first one free again");
    assert_eq!(
        wind_get_rect(&mut pb, again, WF_WXYWH),
        GL_RFULL,
        "wm_delete writes w_setsize(WS_WORK, w_handle, &gl_rfull) (aes/gemwmlib.c:1480) \
         and wm_create never writes WS_WORK (:1377-1379, :205-228)"
    );

    wind_new(&mut pb);
}

/// `wm_delete` runs its four `w_setsize` calls whether or not it had to close
/// the window first: the `VF_ISOPEN` test guards only the `wm_close`
/// (`:1473-1474`), and the writes at `:1477-1480` sit below it unconditionally.
#[test]
fn deleting_a_never_opened_window_arms_the_slot_just_the_same() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = wind_create(&mut pb, KIND, FULL);
    assert_eq!(wind_delete(&mut pb, h), 1);

    let again = wind_create(&mut pb, KIND, FULL);
    assert_eq!(
        wind_get_rect(&mut pb, again, WF_WXYWH),
        GL_RFULL,
        "the rectangle writes at :1477-1480 are not under the VF_ISOPEN test"
    );

    wind_new(&mut pb);
}

/// The guard against the other three leaking. `wm_delete` leaves `gl_rscreen`
/// in `WS_CURR`/`WS_PREV` and `gl_rfull` in `WS_FULL` (`:1477-1479`), and
/// `wm_create` overwrites all three (`:1377-1379`). If it ever stopped doing
/// so, a created-but-unopened window would claim the whole screen — and
/// `find_handle`, `border_pieces` and every hit test read `WS_CURR`.
#[test]
fn the_freed_slots_screen_rectangles_do_not_survive_the_next_create() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = wind_create(&mut pb, KIND, FULL);
    wind_open(&mut pb, h, OPEN_R);
    assert_eq!(wind_delete(&mut pb, h), 1);

    let again = wind_create(&mut pb, KIND, FULL);
    let curr = wind_get_rect(&mut pb, again, WF_CXYWH);
    let prev = wind_get_rect(&mut pb, again, WF_PXYWH);
    let full = wind_get_rect(&mut pb, again, WF_FXYWH);

    assert_eq!(
        curr, GL_RZERO,
        "w_setsize(WS_CURR, i, &gl_rzero) (:1377); the freed slot held \
         gl_rscreen {GL_RSCREEN:?}"
    );
    assert_eq!(
        prev, GL_RZERO,
        "w_setsize(WS_PREV, i, &gl_rzero) (:1378); the freed slot held \
         gl_rscreen {GL_RSCREEN:?}"
    );
    assert_eq!(
        full, FULL,
        "w_setsize(WS_FULL, i, pt) (:1379); the freed slot held gl_rfull {GL_RFULL:?}"
    );

    wind_new(&mut pb);
}

/// While the slot is free none of the four rectangles is readable: `get_pwin`
/// refuses a handle without `VF_INUSE` (`:1391-1404`) and `wm_get` returns
/// FALSE before the field switch for every rectangle field (`:1513-1524`). So
/// what `wm_delete` writes can only ever be seen through the slot's next
/// occupant — which is why the three above are invisible and `WS_WORK` is not.
#[test]
fn a_freed_handle_answers_no_rectangle_at_all() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = wind_create(&mut pb, KIND, FULL);
    wind_open(&mut pb, h, OPEN_R);
    assert_eq!(wind_delete(&mut pb, h), 1);

    for field in [WF_WXYWH, WF_CXYWH, WF_PXYWH, WF_FXYWH] {
        assert_eq!(
            wind_get_ret(&mut pb, h, field),
            0,
            "wind_get({h}, {field}) on a freed handle must fail at get_pwin"
        );
    }

    wind_new(&mut pb);
}

/// The free-slot rule `wm_new` has to obey: it closes and deletes the windows
/// that are **in use** and leaves the rest alone —
/// `if (D.w_win[wh].w_flags & VF_INUSE) wm_delete(wh);` (`:1889-1890`). A slot
/// an earlier `wind_delete` already freed therefore keeps the `gl_rfull` that
/// delete put in it; a sweep that blanked the whole table would take it away
/// again, which is exactly how the defect A33 describes used to read.
#[test]
fn a_sweep_does_not_disturb_a_slot_that_is_already_free() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = wind_create(&mut pb, KIND, FULL);
    wind_open(&mut pb, h, OPEN_R);
    assert_eq!(wind_delete(&mut pb, h), 1);

    // Nothing is in use now, so this sweep has nothing to delete — and must
    // leave the slot it finds free untouched.
    wind_new(&mut pb);

    let again = wind_create(&mut pb, KIND, FULL);
    assert_eq!(
        wind_get_rect(&mut pb, again, WF_WXYWH),
        GL_RFULL,
        "wm_new's VF_INUSE test (:1889) means a free slot is not swept a second time"
    );

    wind_new(&mut pb);
}
