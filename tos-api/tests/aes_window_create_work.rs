//! `wind_create` may not write the window's work rectangle (`WS_WORK`).
//!
//! `wm_create` writes exactly three slots and no more:
//!
//! ```c
//! w_setup(rlr, i, kind);
//! w_setsize(WS_CURR, i, &gl_rzero);
//! w_setsize(WS_PREV, i, &gl_rzero);
//! w_setsize(WS_FULL, i, pt);
//! ```
//!
//! (`references/emutos/aes/gemwmlib.c:1370-1385`, pinned tree
//! `1a7484aba1bfba23e2c0899e243b6d8177fe41df`). `w_setup` (`:205-228`) does not
//! touch `w_work` either — it is not among the fields that function assigns.
//! In the whole AES `WS_WORK` is written in exactly three places: `wm_init` for
//! the desktop window (`:1294`, `= gl_rfull`), `wm_delete` (`:1480`,
//! `= gl_rfull`) and `draw_change`/`w_move` (`:1073-1077`,
//! `= wm_calc(WC_WORK, kind, WS_CURR)`). `wm_get(WF_WXYWH)` is a plain
//! `w_getsize(WS_WORK, …)` (`:1529`, `:236-262`) — it reads the stored slot and
//! computes nothing.
//!
//! A window that has been created but never opened therefore answers
//! `wind_get(WF_WXYWH)` with whatever its **slot** was left holding, which is a
//! value no argument of `wind_create` can influence.
//!
//! **Measured.** `tosdump/cases/aes-r3-wind-state.tsv` (39 cases, CRC
//! `979FAC60`) cases 340 and 344 against `tosdump/reference/EMUTOS14-AES-r3.BIN`
//! (EmuTOS 1.4, TT-medium 640×480/4 planes — the geometry this harness
//! supplies): both answer `0,19,640,461`, which is `gl_rfull`
//! (`aes/gemgraf.c:424`) and is independent of the `kind` `0x0FEF` and the
//! maximum extent `10,20,300,200` the preludes create with. NewTOS answered
//! `11,43,276,154` — exactly `wind_calc(WC_WORK, 0x0FEF, 10,20,300,200)`, the
//! table's own oracle case 337.
//!
//! Ticket: `TASK/A34.md`.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};

// ── the AES calls this file issues ───────────────────────────────────────

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_CLOSE: i16 = 102;
const OP_WIND_GET: i16 = 104;
const OP_WIND_CALC: i16 = 108;
const OP_WIND_NEW: i16 = 109;

const WF_WXYWH: i16 = 4;
const WF_CXYWH: i16 = 5;
const WF_FXYWH: i16 = 7;
const WF_FIRSTXYWH: i16 = 11;

/// `WC_BORDER` / `WC_WORK` (`include/aesdefs.h:555-556`).
const WC_WORK: i16 = 1;

type R = (i16, i16, i16, i16);

/// The pair the measured table creates with: every gadget bit except `INFO`,
/// maximum extent `10,20,300,200` (cases 339/342 of
/// `tosdump/cases/aes-r3-wind-state.tsv`).
const KIND_A: i16 = 0x0FEF;
const FULL_A: R = (10, 20, 300, 200);

/// A deliberately different pair — `NAME|CLOSER|MOVER` and a different
/// rectangle. Its `wm_calc(WC_WORK, …)` is a different rectangle from
/// `KIND_A`/`FULL_A`'s, so a `wind_create` that derives the work area from its
/// arguments cannot answer the same thing for both.
const KIND_B: i16 = 0x000B;
const FULL_B: R = (40, 60, 200, 120);

/// The rectangle a window is opened at in the open-path guard below.
const OPEN_R: R = (30, 40, 320, 240);

/// `gl_rfull` at this harness' geometry — the screen minus the menu row
/// (`r_set(&gl_rfull, 0, gl_hbox, gl_width, gl_height - gl_hbox)`,
/// `aes/gemgraf.c:424`), and the value cases 340/344 of the dump hold.
const GL_RFULL: R = (0, 19, 640, 461);

// ── the calls ────────────────────────────────────────────────────────────

/// `wind_new` — the reset `tosdump/cases/aes-r3-wind-state.tsv` declares on its
/// `!reset` line, issued before and after every case (`aes/gemwmlib.c:1875-1893`,
/// `tosdump/core/runner_aes.c:150-154,218-227`). `OUT_LEN` 0 is not cosmetic:
/// the dispatcher copies `OUT_LEN` words back out of an uninitialised local
/// (`aes/gemsuper.c:430-432,444-445`).
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

/// `wind_close` on a window that was never opened — `wm_close` returns FALSE
/// before it touches a rectangle (`aes/gemwmlib.c:1444-1447`).
fn wind_close(pb: &mut Pb, handle: i16) -> i16 {
    pb.poison();
    pb.intin[0] = handle;
    pb.call(OP_WIND_CLOSE, 1, 1, 0, 0);
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

/// `wind_calc(WC_WORK, kind, x, y, w, h)` — the machine's own oracle for "the
/// work area of this border rectangle", the table's case 337.
fn wind_calc_work(pb: &mut Pb, kind: i16, r: R) -> R {
    pb.poison();
    pb.intin[0] = WC_WORK;
    pb.intin[1] = kind;
    pb.intin[2] = r.0;
    pb.intin[3] = r.1;
    pb.intin[4] = r.2;
    pb.intin[5] = r.3;
    pb.call(OP_WIND_CALC, 6, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_calc(WC_WORK, {kind:#06x}, {r:?}) returned FALSE");
    (pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4])
}

/// Create one window and read its work rectangle, from a **defined slot
/// history**: a window created in the same slot and swept away again.
///
/// The history is not decoration. EmuTOS' answer here is the rectangle the
/// slot's *previous* life left — `wm_delete` writes `WS_WORK = gl_rfull`
/// (`aes/gemwmlib.c:1480`) and nothing in the create path overwrites it — so a
/// virgin slot and a recycled one legitimately answer differently. Two
/// measurements are comparable only if both start from the same one; priming
/// with an identical create + sweep is what makes that true whatever the sweep
/// leaves behind.
fn work_after_bare_create(pb: &mut Pb, kind: i16, full: R) -> R {
    wind_new(pb);
    wind_create(pb, KIND_A, FULL_A);
    wind_new(pb);

    let h = wind_create(pb, kind, full);
    wind_get_rect(pb, h, WF_WXYWH)
}

// ── the tests ────────────────────────────────────────────────────────────

/// **The defect.** `wm_create`'s arguments reach `WS_CURR`, `WS_PREV` and
/// `WS_FULL` and nothing else, so the work rectangle of a created-but-unopened
/// window cannot be a function of `kind` or of the maximum extent. EmuTOS
/// answers the same rectangle for both pairs (`gl_rfull`, cases 340/344 of the
/// dump); a `wind_create` that runs `wm_calc(WC_WORK, kind, full)` answers two
/// different ones.
#[test]
fn the_work_rect_of_a_created_window_does_not_depend_on_kind_or_maximum_extent() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    let a = work_after_bare_create(&mut pb, KIND_A, FULL_A);
    let b = work_after_bare_create(&mut pb, KIND_B, FULL_B);

    assert_eq!(
        a, b,
        "wind_get(WF_WXYWH) on a created-but-unopened window answered {a:?} for \
         kind {KIND_A:#06x}/{FULL_A:?} and {b:?} for kind {KIND_B:#06x}/{FULL_B:?}. \
         wm_create writes WS_CURR, WS_PREV and WS_FULL only \
         (aes/gemwmlib.c:1370-1385) and w_setup does not touch w_work (:205-228), \
         so neither argument may reach the work rectangle"
    );

    wind_new(&mut pb);
}

/// The same defect stated against the machine's own oracle: the answer may not
/// be `wind_calc(WC_WORK, kind, full)`. That is the number the dump caught —
/// NewTOS answered `11,43,276,154` where EmuTOS answered `0,19,640,461`, and
/// `11,43,276,154` is exactly this oracle for the table's create arguments
/// (case 337 against cases 340/344).
#[test]
fn a_created_window_does_not_answer_wind_calc_of_its_maximum_extent() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    let oracle = wind_calc_work(&mut pb, KIND_A, FULL_A);
    let work = work_after_bare_create(&mut pb, KIND_A, FULL_A);

    assert_ne!(
        work, oracle,
        "wind_get(WF_WXYWH) on a created-but-unopened window answered \
         wind_calc(WC_WORK, {KIND_A:#06x}, {FULL_A:?}) = {oracle:?}. WS_WORK is \
         written by wm_init (:1294), wm_delete (:1480) and w_move (:1073-1077) \
         and by nothing in the create path"
    );

    wind_new(&mut pb);
}

/// `wind_close` on a window that was created and never opened returns FALSE
/// before `wm_close` reaches `draw_change` (`aes/gemwmlib.c:1444-1447`), so it
/// may not move the work rectangle either — case 344 of the dump reads the same
/// value as case 340.
#[test]
fn closing_a_never_opened_window_leaves_its_work_rect_where_the_create_found_it() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    wind_new(&mut pb);
    let h = wind_create(&mut pb, KIND_A, FULL_A);
    let before = wind_get_rect(&mut pb, h, WF_WXYWH);

    let ret = wind_close(&mut pb, h);
    assert_eq!(ret, 0, "wind_close on a never-opened window must return FALSE (:1446-1447)");

    let after = wind_get_rect(&mut pb, h, WF_WXYWH);
    assert_eq!(
        after, before,
        "wind_close took its early return, so nothing may have been written: \
         WF_WXYWH was {before:?} before the close and {after:?} after it \
         (dump cases 340 and 344 hold the same rectangle)"
    );

    wind_new(&mut pb);
}

/// The guard against over-removing: `w_move` **does** write `WS_WORK`
/// (`aes/gemwmlib.c:1073-1077`), so an opened window still answers
/// `wm_calc(WC_WORK, kind, WS_CURR)` — the reference case 256 of
/// `tosdump/cases/aes-r2-wind.tsv`, restated here so that a change to the
/// create path cannot silently take the recompute with it.
#[test]
fn an_opened_window_still_answers_wind_calc_of_the_rectangle_it_was_opened_at() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    let expect = wind_calc_work(&mut pb, KIND_A, OPEN_R);

    wind_new(&mut pb);
    let h = wind_create(&mut pb, KIND_A, FULL_A);
    wind_open(&mut pb, h, OPEN_R);

    assert_eq!(
        wind_get_rect(&mut pb, h, WF_WXYWH),
        expect,
        "wind_open runs draw_change, which recomputes WS_WORK from the new \
         WS_CURR (aes/gemwmlib.c:1073-1077)"
    );
    // WS_FULL is the create's rectangle and the open does not touch it
    // (`wm_open`, `:1418-1436`) — the create still writes the slot it is
    // supposed to write.
    assert_eq!(
        wind_get_rect(&mut pb, h, WF_FXYWH),
        FULL_A,
        "wm_create's w_setsize(WS_FULL, i, pt) (:1379) still stands"
    );
    // And the current rectangle is the one it was opened at (`:1073`).
    assert_eq!(wind_get_rect(&mut pb, h, WF_CXYWH), OPEN_R);

    wind_new(&mut pb);
}

/// The only other reader of `w_work` that a created-but-unopened window can
/// reach is `w_owns`, the `WF_FIRSTXYWH`/`WF_NEXTXYWH` walk, which clips each
/// owner rectangle against it (`aes/gemwmlib.c:1230-1240,1556-1561`). An
/// unopened window is not in the window tree, so `newrect` gave it no list
/// (`aes/gemwrect.c:165-205`) and the walk ends before the clip is ever
/// evaluated: `w_owns` writes only `g_w = g_h = 0` (`:1240`). Stated here so
/// that changing what the create path leaves in `WS_WORK` is demonstrably
/// invisible on this path rather than merely argued to be.
#[test]
fn a_created_but_unopened_window_owns_no_rectangle_whatever_its_work_rect_holds() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    wind_new(&mut pb);
    let h = wind_create(&mut pb, KIND_A, FULL_A);

    let first = wind_get_rect(&mut pb, h, WF_FIRSTXYWH);
    assert_eq!(
        (first.2, first.3),
        (0, 0),
        "WF_FIRSTXYWH on a created-but-unopened window must be the end marker; \
         x and y have no soll (A15), so only w and h are read here"
    );

    wind_new(&mut pb);
}

/// The positive half of cases 340 and 344: EmuTOS answers `gl_rfull`, because
/// the reset the table declares is `wind_new` → `wm_new` → `wm_delete`, and
/// `wm_delete` writes `w_setsize(WS_WORK, w_handle, &gl_rfull)`
/// (`aes/gemwmlib.c:1480`) into the slot it frees. Both halves are needed for
/// this number: the sweep has to leave `gl_rfull` behind (A33) *and* the create
/// must not overwrite it (A34). It was `#[ignore]`d while only the second half
/// was in place; A33 supplied the first.
#[test]
fn created_after_a_sweep_answers_gl_rfull_like_the_dump() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // Give slot 1 a previous life, so the sweep below has something to delete —
    // the state cases 340/344 are measured in.
    wind_new(&mut pb);
    let h = wind_create(&mut pb, KIND_A, FULL_A);
    wind_open(&mut pb, h, OPEN_R);
    wind_new(&mut pb);

    let h = wind_create(&mut pb, KIND_A, FULL_A);
    assert_eq!(
        wind_get_rect(&mut pb, h, WF_WXYWH),
        GL_RFULL,
        "EMUTOS14-AES-r3.BIN cases 340 and 344"
    );

    wind_new(&mut pb);
}
