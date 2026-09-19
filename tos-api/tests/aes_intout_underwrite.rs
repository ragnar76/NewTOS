//! An AES handler that writes fewer `int_out` words than the caller declared
//! in `OUT_LEN` delivers **0** in the words it did not write — `TASK/A27.md`.
//!
//! EmuTOS' `xif` builds every result in a **local, uninitialised**
//! `WORD int_out[O_SIZE]` and copies `OUT_LEN` words of it back
//! unconditionally (`references/emutos/aes/gemsuper.c:432,444-445`):
//!
//! ```c
//! WORD    int_out[O_SIZE];
//! ...
//! RET_CODE = crysbind(OP_CODE, (AESGLOBAL *)pcrys_blk->global, control, int_in, int_out,
//!                             addr_in);
//! if (OUT_LEN)
//!     memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD));
//! ```
//!
//! Two things follow, and both are pinned here. **Inside `OUT_LEN` the caller
//! never keeps its own word** — every declared word is overwritten, on both
//! machines. **What lands in a word the handler did not write** differs: the
//! supervisor stack on EmuTOS (measured on the TT: `1, 31, 63, 0, 0` for an
//! exhausted rectangle list, `TASK/A15-STATUS-UNFINISHED.MD`; `…, 0, 1` in
//! `int_out[5..6]` for `graf_handle` at `OUT_LEN 7`, case 283 of
//! `tosdump/cases/aes-r4-subfunctions.tsv`), a defined **0** on NewTOS —
//! the sanctioned deviation in `docs/COMPATIBILITY.md` (*"An AES handler that
//! writes fewer than `OUT_LEN` words delivers 0"*), inventory in
//! `docs/analysis/aes-underwriting-handlers-2026-09-19.md`.
//!
//! The rig poisons the caller's `int_out` with `0xDEAD` before every call
//! (as `tosdump/core/runner_aes.c` does), so three outcomes are told apart in
//! each word: the handler's value, the poison (word not delivered), and 0
//! (word delivered, not written by the handler). A word past `OUT_LEN` must
//! still be poison — the boundary itself is EmuTOS', not ours.
//!
//! Red counter-check (done once, by hand, 2026-09-19): with the zeroing in
//! `tos_api::aes::dispatch` replaced by a `0x7777` fill — a stand-in for
//! "whatever the stack held" — every test below fails on exactly the words
//! it names, and on no other.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK, POISON};

const OP_OBJC_SYSVAR: i16 = 48;
const OP_GRAF_HANDLE: i16 = 77;
const OP_WIND_GET: i16 = 104;

const WF_FXYWH: i16 = 7;
const WF_HSLIDE: i16 = 8;
const WF_COLOR: i16 = 18;
const WF_DCOLOR: i16 = 19;

/// The desktop window, EmuTOS `DESKWH` — real, handle-valid, and its gadget
/// colours are the `wm_start` defaults (`w_setup`, `gemwmlib.c:222-225`).
const DESKWH: i16 = 0;
/// `W_NAME` (`aes/gemlib.h:41`) — the one gadget whose topped colour differs
/// from its untopped colour by default (`gemwmlib.c:1347-1352`), so the two
/// colour words cannot be confused with each other.
const W_NAME: i16 = 3;
/// `wm_start`'s defaults for `W_NAME`: `0x1101 | 0xa0` topped, `0x1101`
/// untopped (`gemwmlib.c:1347-1352`).
const W_NAME_TOPPED: i16 = 0x11a1;
const W_NAME_UNTOPPED: i16 = 0x1101;

/// `objc_sysvar`'s `LK3DIND` selector (`aes/obdefs.h`), and the set mode.
const LK3DIND: i16 = 1;
const SYSVAR_SET: i16 = 1;

fn lock() -> std::sync::MutexGuard<'static, ()> {
    AES_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// `wind_get(handle, field[, gadget])` declared with `OUT_LEN = out_len`,
/// poisoned first. `IN_LEN` follows gemlib: 3 for the colour fields, 2
/// otherwise (`a_wind_get.c:338-348`).
fn wind_get(pb: &mut Pb, handle: i16, field: i16, gadget: i16, out_len: i16) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    let in_len = if field == WF_COLOR || field == WF_DCOLOR {
        pb.intin[2] = gadget;
        3
    } else {
        2
    };
    pb.call(OP_WIND_GET, in_len, out_len, 0, 0);
}

/// `wind_get(0, WF_COLOR, W_NAME)` at `OUT_LEN 6`: the handler writes
/// `int_out[0]` and the two colour words at `int_out[2..3]`
/// (`poutwds[1..2]`, `gemwmlib.c:1569-1570`, D-095) — and nothing else.
/// `int_out[1]`, `[4]` and `[5]` are inside the declaration, so the poison
/// must be gone from them, and what replaces it is 0, not the stack.
#[test]
fn wind_get_wf_color_delivers_zero_in_the_three_words_it_never_writes() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();

    wind_get(&mut pb, DESKWH, WF_COLOR, W_NAME, 6);

    assert_eq!(pb.intout[0], 1, "wind_get(0, WF_COLOR) must answer TRUE");
    assert_eq!(
        (pb.intout[2], pb.intout[3]),
        (W_NAME_TOPPED, W_NAME_UNTOPPED),
        "the colour words sit at int_out[2..3] (D-095)"
    );
    for i in [1, 4, 5] {
        assert_ne!(pb.intout[i], POISON, "int_out[{i}] is inside OUT_LEN 6 and must be delivered");
        assert_eq!(pb.intout[i], 0, "int_out[{i}] was not written by the handler and must read 0");
    }
    assert_eq!(pb.intout[6], POISON, "int_out[6] is past OUT_LEN 6 and must stay the caller's");

    // The same two words come back for the defaults the desktop window was
    // set up from (`w_setup` copies `gl_wtcolor`/`gl_wbcolor`, `:222-225`),
    // and `WF_DCOLOR` under-writes in the same three slots (`:1574-1575`).
    wind_get(&mut pb, DESKWH, WF_DCOLOR, W_NAME, 6);
    assert_eq!(pb.intout[0], 1);
    assert_eq!((pb.intout[2], pb.intout[3]), (W_NAME_TOPPED, W_NAME_UNTOPPED));
    assert_eq!((pb.intout[1], pb.intout[4], pb.intout[5]), (0, 0, 0));
    assert_eq!(pb.intout[6], POISON);
}

/// `graf_handle` at `OUT_LEN 7` — case 283 of `aes-r4-subfunctions.tsv`.
/// The handler writes `int_out[0..4]` (`gemsuper.c:288-292`); the TT
/// answered `0, 1` in `int_out[5..6]` (its stack), NewTOS answers `0, 0`.
/// gemlib's `graf_xhandle` declares `OUT_LEN 6` and *pre-zeroes*
/// `aes_intout[5]` "for AES that doesn't support this extension"
/// (`a_graf_xhandle.c:46-49`) — the value it wanted is the one it gets here.
#[test]
fn graf_handle_at_out_len_7_delivers_zero_in_the_two_words_past_its_metrics() {
    let _guard = lock();
    let ctx = context();
    let mut pb = Pb::new();

    pb.poison();
    pb.call(OP_GRAF_HANDLE, 0, 7, 0, 0);

    assert_eq!(pb.intout[0], ctx.phys_handle, "graf_handle answers the physical workstation handle");
    for i in 1..=4 {
        assert_ne!(pb.intout[i], POISON, "int_out[{i}] is a metric the handler writes");
        assert!(pb.intout[i] > 0, "int_out[{i}] is a cell/box size, so positive");
    }
    assert_eq!((pb.intout[5], pb.intout[6]), (0, 0), "int_out[5..6]: delivered, unwritten, therefore 0");

    // The boundary is still exactly OUT_LEN: at 5, the same two words are
    // not delivered at all and keep the caller's poison (case 282).
    pb.poison();
    pb.call(OP_GRAF_HANDLE, 0, 5, 0, 0);
    assert_eq!(pb.intout[0], ctx.phys_handle);
    assert_eq!((pb.intout[5], pb.intout[6]), (POISON, POISON));
}

/// `wind_get(0, WF_HSLIDE)` at `OUT_LEN 5` — the pattern of cases 415/418
/// (`WF_HSLIDE`/`WF_VSLIDE`), where the TT delivered the *previous* case's
/// `WF_FXYWH` rectangle words in `int_out[2..4]` (`20, 300, 200`). The
/// handler writes `poutwds[0]` only (`gemwmlib.c:1542-1543`). Run the same
/// sequence: a rectangle call first, so that a leak of the previous answer
/// would be recognisable, then the scalar call — `int_out[2..4]` must be 0,
/// neither the poison nor the rectangle.
#[test]
fn wind_get_wf_hslide_at_out_len_5_delivers_zero_not_the_previous_rectangle() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();

    wind_get(&mut pb, DESKWH, WF_FXYWH, 0, 5);
    assert_eq!(pb.intout[0], 1);
    let rect = [pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4]];
    assert!(rect[2] > 0 && rect[3] > 0, "the desktop's full rect has a size: {rect:?}");

    wind_get(&mut pb, DESKWH, WF_HSLIDE, 0, 5);
    assert_eq!(pb.intout[0], 1);
    assert_eq!(pb.intout[1], 0, "w_setup leaves the slider at the left (`gemwmlib.c:217`)");
    for i in 2..=4 {
        assert_ne!(pb.intout[i], POISON, "int_out[{i}] is inside OUT_LEN 5 and must be delivered");
        assert_ne!(pb.intout[i], rect[i - 1], "int_out[{i}] must not carry the previous call's answer");
        assert_eq!(pb.intout[i], 0, "int_out[{i}] was not written by the handler and must read 0");
    }
    assert_eq!((pb.intout[5], pb.intout[6]), (POISON, POISON));
}

/// `objc_sysvar` in set mode at gemlib's `OUT_LEN 3` (`a_objc_sysvar.c:71`,
/// which reads `int_out[1..2]` back in *both* modes, `:83-87`): `ob_sysvar`
/// never touches `out1`/`out2` when setting (`aes/gemoblib.c:1355-1390`).
/// Both values `-1` leave the variables alone (`:1359-1362`), so the AES
/// state is unchanged by this test.
#[test]
fn objc_sysvar_set_mode_at_out_len_3_delivers_zero_in_the_two_output_words() {
    let _guard = lock();
    context();
    let mut pb = Pb::new();

    pb.poison();
    pb.intin[0] = SYSVAR_SET;
    pb.intin[1] = LK3DIND;
    pb.intin[2] = -1;
    pb.intin[3] = -1;
    pb.call(OP_OBJC_SYSVAR, 4, 3, 0, 0);

    assert_eq!(pb.intout[0], 1, "a set of a known selector answers 1 (`gemoblib.c:1389`)");
    assert_eq!((pb.intout[1], pb.intout[2]), (0, 0), "int_out[1..2]: delivered, unwritten, therefore 0");
    assert_eq!(pb.intout[3], POISON, "int_out[3] is past OUT_LEN 3");
}
