//! The AES dispatcher must hand a manager **only** the input words the caller
//! declared — `TASK/A19.md`.
//!
//! EmuTOS' `xif` never lets a manager touch the caller's arrays. It copies the
//! inputs **once**, into its own stack buffers, and clamps while copying
//! (`references/emutos/aes/gemsuper.c:435-439`):
//!
//! ```c
//! memcpy(control, pcrys_blk->control, C_SIZE*sizeof(WORD));
//! if (IN_LEN)
//!     memcpy(int_in, pcrys_blk->intin, min(IN_LEN,I_SIZE)*sizeof(WORD));
//! if (AIN_LEN)
//!     memcpy(addr_in, pcrys_blk->addrin, min(AIN_LEN,AI_SIZE)*sizeof(LONG));
//! ```
//!
//! `int_in[I_SIZE]` and `addr_in[AI_SIZE]` are `xif`'s own locals
//! (`:431`, `:433`), so a manager that reaches past `IN_LEN` / `AIN_LEN` reads
//! **that buffer**, never the application's memory. NewTOS used to read the
//! caller's arrays directly and unclamped, which on a machine with two address
//! spaces is not a stale-value question but a read of application memory the
//! call never declared.
//!
//! What lies past the boundary is EmuTOS' indeterminate `auto` storage — no
//! value to reproduce, exactly as for `int_out[O_SIZE]`, which
//! [`tos_api::aes::dispatch`] already zeroes for the same reason. NewTOS zeroes
//! the two input buffers too, so a manager reading past the declaration sees a
//! defined 0 instead of somebody else's bytes. These tests pin the boundary
//! with a **recognisable non-zero** pattern on the far side, so a passing run
//! cannot be an accident of everything being zero anyway.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};

const OP_SCRP_READ: i16 = 80;
const OP_SCRP_WRITE: i16 = 81;
const OP_WIND_CREATE: i16 = 100;
const OP_WIND_GET: i16 = 104;
const OP_WIND_NEW: i16 = 109;

const WF_FXYWH: i16 = 7;

/// `wind_create`'s border kind — every gadget, as the r2 window table uses.
const KIND: i16 = 0x0FEF;

/// The rectangle words a *conforming* `wind_create` caller would put in
/// `int_in[1..4]`. Deliberately all four distinct, all non-zero, and none of
/// them a value `wm_create` could produce on its own — so "the manager saw the
/// caller's word" and "the manager saw the cleared buffer" can never be
/// confused, in either direction.
const MARK: [i16; 4] = [0x1234, 0x5678, 0x4321, 0x0765];

/// `wind_new` (109) — the AES-wide window reset every window case starts from.
fn wind_new(pb: &mut Pb) {
    pb.poison();
    pb.call(OP_WIND_NEW, 0, 0, 0, 0);
}

/// `wind_create(KIND, MARK)` laid in `int_in[0..4]` but **declared** with
/// `IN_LEN = in_len`, and the handle it returned.
fn create_declaring(pb: &mut Pb, in_len: i16) -> i16 {
    pb.poison();
    pb.intin[0] = KIND;
    pb.intin[1] = MARK[0];
    pb.intin[2] = MARK[1];
    pb.intin[3] = MARK[2];
    pb.intin[4] = MARK[3];
    pb.call(OP_WIND_CREATE, in_len, 1, 0, 0);
    let h = pb.intout[0];
    assert!(h >= 1, "wind_create(IN_LEN {in_len}) returned {h}");
    h
}

/// `wind_get(handle, WF_FXYWH)` — the `WS_FULL` rectangle `wm_create` stored
/// (`references/emutos/aes/gemwmlib.c:1380`, read back at `:1538-1541`). This
/// is the observable that says which words reached the manager: `wind_create`
/// is the one opcode that takes four caller words and parks them verbatim in
/// window state, so reading them back reports exactly what it was handed.
fn full_rect(pb: &mut Pb, handle: i16) -> [i16; 4] {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = WF_FXYWH;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_get({handle}, WF_FXYWH) must succeed");
    [pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4]]
}

/// The whole point, in one call: `IN_LEN 1` promises the manager exactly one
/// word, so the four rectangle words behind it must not reach `wm_create` —
/// even though the caller's array demonstrably holds them.
///
/// The reference measured this very shape on the TT: case 140 of
/// `tosdump/cases/aes-r2-wind.tsv`, "wind_create IN_LEN 1". There the words
/// past the declaration were EmuTOS' own stack, which is why the case only
/// pins the returned handle; here the caller's array holds [`MARK`], so the
/// stored rectangle tells the two behaviours apart.
#[test]
fn wind_create_with_in_len_1_must_not_see_the_four_words_behind_it() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = create_declaring(&mut pb, 1);
    assert_eq!(
        full_rect(&mut pb, h),
        [0, 0, 0, 0],
        "wind_create declared IN_LEN 1; int_in[1..4] were {MARK:#x?} in the \
         caller's array and must not have reached wm_create — a rectangle of \
         {MARK:#x?} here means the manager read past the declaration, straight \
         out of application memory (gemsuper.c:436-437)"
    );

    wind_new(&mut pb);
}

/// The clamp must cut at `IN_LEN`, not before it and not after it: with
/// `IN_LEN 3` the first two rectangle words are declared and must arrive, the
/// last two are not and must not.
#[test]
fn the_cut_is_exactly_at_in_len_not_one_word_early_or_late() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = create_declaring(&mut pb, 3);
    assert_eq!(
        full_rect(&mut pb, h),
        [MARK[0], MARK[1], 0, 0],
        "IN_LEN 3 declares int_in[0..2]: x and y must arrive, w and h must not"
    );

    wind_new(&mut pb);
}

/// A fully declared call is untouched by the clamp — the regression guard for
/// every ordinary caller, and the proof that the two tests above fail for the
/// boundary and not because `wind_create` stopped storing its rectangle.
#[test]
fn a_fully_declared_call_still_receives_every_word() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = create_declaring(&mut pb, 5);
    assert_eq!(full_rect(&mut pb, h), MARK, "IN_LEN 5 declares all five words");

    // `I_SIZE` (16, `include/aesdefs.h:122`) is the other half of `min()`: an
    // over-declaring caller is clamped down to the buffer that exists, and the
    // words it *did* declare still arrive.
    let h = create_declaring(&mut pb, 30);
    assert_eq!(
        full_rect(&mut pb, h),
        MARK,
        "IN_LEN 30 > I_SIZE must clamp to 16 words, not truncate the call"
    );

    wind_new(&mut pb);
}

/// `IN_LEN 0` is EmuTOS' `if (IN_LEN)` guard: not one word is copied, so a
/// manager that reads `int_in[0]` sees the cleared buffer. `wind_create`'s
/// border kind is that word, so the created window gets kind 0 — no gadgets —
/// rather than the caller's `KIND`.
#[test]
fn in_len_0_copies_nothing_at_all() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    wind_new(&mut pb);

    let h = create_declaring(&mut pb, 0);
    assert_eq!(
        full_rect(&mut pb, h),
        [0, 0, 0, 0],
        "IN_LEN 0 declares no input words at all (gemsuper.c:436)"
    );

    wind_new(&mut pb);
}

/// The same clamp on the address side — `if (AIN_LEN) memcpy(addr_in,
/// pcrys_blk->addrin, min(AIN_LEN,AI_SIZE)*sizeof(LONG))`
/// (`gemsuper.c:438-439`).
///
/// `scrp_write` takes its one argument in `addr_in[0]`
/// (`aes/gemsuper.c:312`, `sc_write((char *)SC_PATH)`), and `scrp_read` hands
/// the stored path back through `addr_in[0]` — so writing, then reading, shows
/// exactly which pointer the manager was given. With `AIN_LEN 0` no slot is
/// declared, the manager must see a null pointer, and NewTOS' `scrp_write`
/// answers a null source by clearing the path (`tos_api::aes::shell`'s
/// `cstr_in`) rather than by dereferencing whatever the caller happened to
/// leave in the slot.
#[test]
fn scrp_write_with_ain_len_0_must_not_see_the_pointer_behind_it() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    let declared: [u8; 16] = *b"C:\\A19DECL.SCP\0\0";
    let behind: [u8; 16] = *b"C:\\A19PAST.SCP\0\0";
    let mut out = [0u8; 128];

    // Baseline: a conforming caller declares its one slot and is obeyed.
    pb.poison();
    pb.addrin[0] = declared.as_ptr() as u64;
    pb.call(OP_SCRP_WRITE, 0, 1, 1, 0);
    pb.poison();
    pb.addrin[0] = out.as_mut_ptr() as u64;
    pb.call(OP_SCRP_READ, 0, 1, 1, 0);
    assert_eq!(
        &out[..14],
        &declared[..14],
        "scrp_write(AIN_LEN 1) must store the declared path"
    );

    // The boundary: the slot holds a perfectly valid, recognisable pointer,
    // and the call declares none. The manager must not be handed it.
    pb.poison();
    pb.addrin[0] = behind.as_ptr() as u64;
    pb.call(OP_SCRP_WRITE, 0, 1, 0, 0);
    out.fill(0xAA);
    pb.poison();
    pb.addrin[0] = out.as_mut_ptr() as u64;
    pb.call(OP_SCRP_READ, 0, 1, 1, 0);
    assert_eq!(
        out[0], 0,
        "scrp_write declared AIN_LEN 0; addr_in[0] pointed at {:?} and must not \
         have reached sc_write — a stored path here means the manager read an \
         address slot the call never declared (gemsuper.c:438-439)",
        core::str::from_utf8(&behind[..14]).unwrap()
    );

    // Leave the scrap directory as this test found it — it is process-wide AES
    // state, not per-case state.
    pb.poison();
    pb.addrin[0] = declared.as_ptr() as u64;
    pb.call(OP_SCRP_WRITE, 0, 1, 1, 0);
}
