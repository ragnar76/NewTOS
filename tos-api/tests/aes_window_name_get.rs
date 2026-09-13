//! `wind_get(WF_NAME)` and `wind_get(WF_INFO)` are fields the AES does not
//! answer.
//!
//! EmuTOS handles the two title fields **only in `wm_set`**
//! (`references/emutos/aes/gemwmlib.c:1660` `WF_NAME`, `:1668` `WF_INFO`; the
//! pinned tree `1a7484aba1bfba23e2c0899e243b6d8177fe41df`). `wm_get`'s switch
//! has no arm for either of them, so both fall into `default: return FALSE`
//! (`:1591-1592`) — the function returns before a single word of `poutwds` is
//! written, and the caller's string buffer is never looked at at all.
//!
//! gemlib *declares* the read side (`wind_get(handle, WF_NAME, &lo, &hi, …)`,
//! `a_wind_get.c:344` packs a `char*` into `int_in[2..]`), and NewTOS used to
//! honour that declaration: it copied the stored title out and returned 1.
//! That was following the wrong authority. gemlib is the client-side binding;
//! it can declare any call it likes, but what a call *does* is decided by the
//! AES on the other side of the trap, and on real TOS that AES refuses the
//! field. A program built against the gemlib header and run on a TT gets
//! FALSE and an untouched buffer.
//!
//! Measured, before the repair: `tosdump/cases/aes-m05-addrin.tsv` case 102
//! (`wind_get WF_NAME into buffer 0`, on a window whose title was set by the
//! preceding prelude) — EmuTOS 1.4 on the reference TT recorded `int_out[0] =
//! 0`, NewTOS answered `1`
//! (`measurement-2026-09-13-regression/aes-m05-addrin/DIFF-vs-emutos.txt`).
//!
//! The buffer assertion is checked **before** the return code on purpose; it
//! is the load-bearing half. "Returns 0 but writes anyway" would still be
//! wrong: a caller that ignores the return code — which is exactly the caller
//! gemlib's header invites — must find its buffer as it left it, not silently
//! rewritten.
//!
//! Ticket: `TASK/A42.md` (finding A-1 of the 2026-09-12 sub-function audit,
//! `docs/api/aes/_full-function-inventory.md`, part II).

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK, POISON};

const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_GET: i16 = 104;
const OP_WIND_SET: i16 = 105;
const OP_WIND_NEW: i16 = 109;

const WF_KIND: i16 = 1;
const WF_NAME: i16 = 2;
const WF_INFO: i16 = 3;
const WF_WXYWH: i16 = 4;

const KIND: i16 = 0x0FEF;

/// The pattern the caller's buffer carries into the call. Chosen so that a
/// partial copy, a stray NUL or a full overwrite are all visible, and so that
/// it can never be mistaken for the title we set.
const PATTERN: [u8; 32] = [0x5A; 32];

fn wind_new(pb: &mut Pb) {
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);
}

/// Create and open one window, as preludes 99/100 of the m05 table do.
fn open_window(pb: &mut Pb) -> i16 {
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
    assert_eq!(pb.intout[0], 1, "wind_open({h}) failed");
    h
}

/// Pack a pointer into `int_in[word..]` the way a classic-AES caller does
/// (`gemlib a_wind_set_str.c:25` / `a_wind_get.c:344`), at the host's pointer
/// width, and leave `ain_len` at 0.
fn pack_ptr(pb: &mut Pb, word: usize, p: u64) {
    for i in 0..(core::mem::size_of::<usize>() / 2) {
        pb.intin[word + i] = ((p >> (16 * i)) & 0xFFFF) as u16 as i16;
    }
}

/// `wind_set(handle, field, str)`, classic-AES packing — the one side of the
/// pair EmuTOS really does implement.
fn wind_set_str(pb: &mut Pb, handle: i16, field: i16, s: *const u8) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pack_ptr(pb, 2, s as u64);
    pb.call(OP_WIND_SET, 6, 1, 0, 0);
}

/// `wind_get(handle, field)` with the buffer in `addr_in[0]` and `ain_len 1` —
/// the native convention, and the one `tosdump/cases/aes-m05-addrin.tsv`
/// case 102 measures (`a0=0`).
fn wind_get_addrin(pb: &mut Pb, handle: i16, field: i16, buf: *mut u8) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.addrin[0] = buf as u64;
    pb.call(OP_WIND_GET, 2, 1, 1, 0);
}

/// The same read with the buffer packed into `int_in[2..]` and `ain_len 0` —
/// the convention gemlib's `wind_get` binding uses.
fn wind_get_packed(pb: &mut Pb, handle: i16, field: i16, buf: *mut u8) {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pack_ptr(pb, 2, buf as u64);
    pb.call(OP_WIND_GET, 6, 1, 0, 0);
}

/// Byte-for-byte: the call must not have touched one byte of it.
fn assert_untouched(buf: &[u8; 32], what: &str) {
    if let Some((i, &b)) = buf.iter().enumerate().find(|(i, &b)| b != PATTERN[*i]) {
        panic!(
            "{what}: the caller's buffer was written at byte {i} (0x{b:02X}, expected \
             0x{:02X}). EmuTOS' wm_get returns from `default:` before poutwds or the \
             string pointer are touched (aes/gemwmlib.c:1591-1592); a refused field \
             must leave the buffer exactly as the caller laid it. Buffer: {buf:02X?}",
            PATTERN[i]
        );
    }
}

/// Both ways of handing `wind_get` a buffer, on a window that really has a
/// title: FALSE, and not one byte written.
#[test]
fn wf_name_is_refused_and_leaves_the_callers_buffer_untouched() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);
    let h = open_window(&mut pb);

    let title = b"ABCDEFGH\0";
    wind_set_str(&mut pb, h, WF_NAME, title.as_ptr());
    assert_eq!(pb.intout[0], 1, "wind_set(WF_NAME) is the half EmuTOS implements");

    let mut buf = PATTERN;
    wind_get_addrin(&mut pb, h, WF_NAME, buf.as_mut_ptr());
    assert_untouched(&buf, "wind_get(WF_NAME) via addr_in[0]");
    assert_eq!(
        pb.intout[0], 0,
        "wind_get({h}, WF_NAME) must answer FALSE: wm_get has no WF_NAME arm and \
         falls into `default: return FALSE` (aes/gemwmlib.c:1591-1592). Measured on \
         EmuTOS 1.4: aes-m05-addrin.tsv case 102, int_out[0] = 0"
    );

    let mut buf = PATTERN;
    wind_get_packed(&mut pb, h, WF_NAME, buf.as_mut_ptr());
    assert_untouched(&buf, "wind_get(WF_NAME) via the packed int_in pointer");
    assert_eq!(
        pb.intout[0], 0,
        "the gemlib packing must not reach a different answer than addr_in does — \
         the AES refuses the field, not the calling convention"
    );

    wind_new(&mut pb);
}

/// `WF_INFO` is the same story one case further down `wm_set`'s switch
/// (`aes/gemwmlib.c:1668`) and equally absent from `wm_get`.
#[test]
fn wf_info_is_refused_and_leaves_the_callers_buffer_untouched() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);
    let h = open_window(&mut pb);

    let info = b"3 items, 12 KB\0";
    wind_set_str(&mut pb, h, WF_INFO, info.as_ptr());
    assert_eq!(pb.intout[0], 1, "wind_set(WF_INFO) is the half EmuTOS implements");

    let mut buf = PATTERN;
    wind_get_addrin(&mut pb, h, WF_INFO, buf.as_mut_ptr());
    assert_untouched(&buf, "wind_get(WF_INFO) via addr_in[0]");
    assert_eq!(
        pb.intout[0], 0,
        "wind_get({h}, WF_INFO) must answer FALSE (aes/gemwmlib.c:1591-1592)"
    );

    let mut buf = PATTERN;
    wind_get_packed(&mut pb, h, WF_INFO, buf.as_mut_ptr());
    assert_untouched(&buf, "wind_get(WF_INFO) via the packed int_in pointer");
    assert_eq!(pb.intout[0], 0, "likewise for the gemlib packing");

    wind_new(&mut pb);
}

/// A refused field writes no `int_out` words either, and the refusal stays
/// inside the two title fields: the rectangle read next door still answers.
#[test]
fn the_refusal_writes_no_words_and_stays_inside_the_title_fields() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);
    let h = open_window(&mut pb);

    let title = b"HIJKLMNO\0";
    wind_set_str(&mut pb, h, WF_NAME, title.as_ptr());

    // OUT_LEN 5, so five words are copied back; `default:` returns before
    // `poutwds` is written, leaving the dispatcher's zeroed local int_out
    // (EmuTOS' `auto WORD int_out[O_SIZE]`, aes/gemsuper.c:432).
    pb.poison();
    pb.intin[0] = h;
    pb.intin[1] = WF_NAME;
    pb.addrin[0] = 0;
    pb.call(OP_WIND_GET, 2, 5, 1, 0);
    assert_eq!(pb.intout[0], 0, "FALSE");
    assert_eq!(
        &pb.intout[1..5],
        &[0, 0, 0, 0],
        "a refused field writes no poutwds words"
    );
    assert_eq!(
        pb.intout[5], POISON,
        "the dispatcher copies back exactly OUT_LEN = 5 words (gemsuper.c:444-445)"
    );

    // WF_KIND (1) is refused for the same reason (AUD-P5-014) — the arm below
    // the title fields, so a mis-placed `break` would show up here.
    pb.poison();
    pb.intin[0] = h;
    pb.intin[1] = WF_KIND;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 0, "WF_KIND stays refused too");

    // WF_WXYWH (4) is one of the four rectangle fields wm_get really answers
    // (`w_getsize`, gemwmlib.c:1528-1541,1596-1597): the repair must not have
    // widened into them.
    pb.poison();
    pb.intin[0] = h;
    pb.intin[1] = WF_WXYWH;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "WF_WXYWH still answers TRUE");
    assert!(pb.intout[3] > 0 && pb.intout[4] > 0, "and still with a real work area");

    wind_new(&mut pb);
}
