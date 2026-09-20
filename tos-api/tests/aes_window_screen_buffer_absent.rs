//! `wind_get(WF_SCREEN)` **without** a screen buffer answers FALSE and writes
//! nothing — the A54 fallback, kept under A57 for the one case in which
//! `gsx_malloc` has nothing to report: the pool could not supply the block at
//! boot (or, as here, the AES runs in a host process that never installed
//! the fixture).
//!
//! EmuTOS in that state answers TRUE with address 0 (`gsx_malloc` leaves
//! `gl_tmp.fd_addr == NULL` when `dos_alloc_anyram` fails and `gsx_mret`
//! reports it, `aes/gemgsxif.c:108-129`) — a pointer a program would then
//! scribble through. NewTOS refuses the field instead, `wm_get`'s own idiom
//! for a field it cannot serve (`default: return FALSE`,
//! `aes/gemwmlib.c:1590-1591`); the deviation is registered in
//! `docs/COMPATIBILITY.md` (A54, narrowed by A57 to this fallback).
//!
//! This binary deliberately takes the harness host **without** `context()`,
//! which is where the fixture is installed; the AES it exercises has no
//! buffer, so what it pins is the fallback and nothing else. The alert and
//! the drop-down still work in that state through a per-save heap copy —
//! `tests/aes_form_alert_cursor_bracket.rs` and
//! `tests/aes_menu_click_ownership.rs` run their rigs without the fixture and
//! cover that path.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{host_only, Pb, AES_LOCK, POISON};

const OP_WIND_GET: i16 = 104;
const OP_WIND_NEW: i16 = 109;
const WF_SCREEN: i16 = 17;

#[test]
fn wf_screen_without_a_buffer_answers_false_and_writes_nothing() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    host_only();
    assert!(
        tos_api::aes::gsxif::gsx_mret().is_none(),
        "this binary must run without the screen-buffer fixture"
    );

    let mut pb = Pb::new();
    pb.poison();
    pb.call(10, 0, 1, 0, 0); // appl_init
    assert!(pb.intout[0] >= 0, "appl_init failed ({})", pb.intout[0]);
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);

    pb.poison();
    pb.intin[0] = 0;
    pb.intin[1] = WF_SCREEN;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(
        pb.intout[0], 0,
        "wind_get(0, WF_SCREEN) must answer FALSE while there is no buffer to report"
    );
    assert_eq!(
        &pb.intout[1..5],
        &[0, 0, 0, 0],
        "the fallback writes no address and no length; these four words are the \
         dispatcher's zeroed local int_out, not an answer"
    );
    assert_eq!(pb.intout[5], POISON, "the dispatcher copies back exactly OUT_LEN = 5 words");
}
