//! `wind_get(WF_SCREEN)` must not claim a buffer NewTOS does not have.
//!
//! EmuTOS answers field 17 out of `gsx_mret` — the address of the menu/alert
//! screen-save buffer in `poutwds[0..1]`, its length in `poutwds[2..3]`, and
//! TRUE (`references/emutos/aes/gemwmlib.c:1563-1565`, `gsx_mret`
//! `aes/gemgsxif.c:125-129`, pinned tree
//! `1a7484aba1bfba23e2c0899e243b6d8177fe41df`). The buffer is allocated **once**
//! at AES start by `gsx_malloc` (`:108-114`, from `sh_main`,
//! `aes/gemshlib.c:236`); the 1.4 ROM on the reference TT reported address
//! `0x01000218`, length `40960` (`measurement-2026-09-12-aes-r4/`, cases 331,
//! 332 and 439 of `tosdump/cases/aes-r4-subfunctions.tsv`, CRC `227422B2`).
//!
//! NewTOS has no such buffer — see the `WF_SCREEN` arm of
//! `tos_api::aes::window::wind_get` and the sanctioned-deviation entry in
//! `docs/COMPATIBILITY.md` for why, and why fabricating one is not a
//! `wind_get` decision. What it used to do was worse than having none: it
//! wrote four zero words and returned **1**, so a caller could not tell "there
//! is no buffer" from "here is one, at address 0". This file pins the honest
//! answer: FALSE, the way `wm_get`'s own `default:` arm refuses a field it does
//! not implement (`gemwmlib.c:1590-1591`).
//!
//! Ticket: `TASK/A54.md` (finding D-2 of
//! `docs/analysis/aes-r4-subfunktionen-einordnung-2026-09-12.md`).

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK, POISON};

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_GET: i16 = 104;
const OP_WIND_NEW: i16 = 109;

const WF_SCREEN: i16 = 17;
const WF_TOP: i16 = 10;
const WF_DCOLOR: i16 = 19;

const KIND: i16 = 0x0FEF;

fn wind_new(pb: &mut Pb) {
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);
}

/// One `wind_get`, laid exactly as the r4 table lays cases 331/332/439:
/// `IN_LEN 2`, `OUT_LEN 5`, everything poisoned first.
fn wind_get5(pb: &mut Pb, handle: i16, field: i16) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
}

/// The three handles the measurement asks WF_SCREEN with: the desktop window
/// (case 331), an invalid one (case 332) and a real open window (case 439).
/// All three must refuse, because the refusal is about the buffer, not the
/// handle.
#[test]
fn wf_screen_refuses_on_every_handle_instead_of_reporting_a_zero_buffer() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    // Case 331: the desktop handle.
    wind_get5(&mut pb, 0, WF_SCREEN);
    assert_eq!(
        pb.intout[0], 0,
        "wind_get(0, WF_SCREEN) must answer FALSE: there is no persistent \
         menu/alert buffer to report, and TRUE would be read as 'here is one'"
    );

    // Case 332: WF_SCREEN needs no valid handle in EmuTOS, so an invalid one
    // must not change the answer either way — the refusal has to come from the
    // missing buffer, not from the handle pre-check.
    wind_get5(&mut pb, -1, WF_SCREEN);
    assert_eq!(pb.intout[0], 0, "wind_get(-1, WF_SCREEN) must answer FALSE too");

    // Case 439: a window that really exists and is open.
    pb.poison();
    pb.intin[0] = KIND;
    pb.intin[1] = 10;
    pb.intin[2] = 20;
    pb.intin[3] = 300;
    pb.intin[4] = 200;
    pb.call(OP_WIND_CREATE, 5, 1, 0, 0);
    let h = pb.intout[0];
    assert!(h >= 1, "wind_create returned {h}");
    pb.poison();
    pb.intin[0] = h;
    pb.intin[1] = 30;
    pb.intin[2] = 40;
    pb.intin[3] = 320;
    pb.intin[4] = 240;
    pb.call(OP_WIND_OPEN, 5, 1, 0, 0);

    wind_get5(&mut pb, h, WF_SCREEN);
    assert_eq!(
        pb.intout[0], 0,
        "wind_get({h}, WF_SCREEN) on an open window must answer FALSE as well"
    );

    wind_new(&mut pb);
}

/// A refused field writes **nothing**: `wm_get`'s `default:` arm returns before
/// `poutwds` is touched (`gemwmlib.c:1590-1591`). What the caller reads back in
/// `int_out[1..4]` is therefore the dispatcher's zeroed local `int_out[O_SIZE]`
/// (`tos_api::aes::dispatch`, EmuTOS' `auto WORD int_out[O_SIZE]`,
/// `aes/gemsuper.c:432`), and nothing past `OUT_LEN` is copied at all.
#[test]
fn wf_screen_writes_no_address_and_no_length() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    wind_get5(&mut pb, 0, WF_SCREEN);
    assert_eq!(pb.intout[0], 0, "FALSE");
    assert_eq!(
        &pb.intout[1..5],
        &[0, 0, 0, 0],
        "the WF_SCREEN arm must not write an address or a length; these four \
         words are the dispatcher's zeroed local int_out, not an answer"
    );
    assert_eq!(
        pb.intout[5], POISON,
        "the dispatcher copies back exactly OUT_LEN = 5 words (gemsuper.c:444-445)"
    );

    wind_new(&mut pb);
}

/// The repair must stay inside field 17. `WF_TOP` and `WF_DCOLOR` are the other
/// two fields `wm_get` answers without a valid handle (`gemwmlib.c:1517-1520`),
/// and both still answer TRUE on the invalid handle case 333 uses — so
/// WF_SCREEN's FALSE did not get routed through the handle pre-check.
#[test]
fn the_other_two_handle_free_fields_still_answer() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    wind_get5(&mut pb, -1, WF_TOP);
    assert_eq!(pb.intout[0], 1, "wind_get(-1, WF_TOP) still needs no handle");

    pb.poison();
    pb.intin[0] = -1;
    pb.intin[1] = WF_DCOLOR;
    pb.intin[2] = 0; // gadget
    pb.call(OP_WIND_GET, 3, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_get(-1, WF_DCOLOR) still needs no handle");

    wind_new(&mut pb);
}
