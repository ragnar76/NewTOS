//! T04 — how a `LONG` (a 32-bit value on the 68k) travels inside the AES
//! `int_in`/`int_out` WORD arrays, pinned at the dispatcher for every kernel
//! reader and writer there is (`docs/analysis/t04-lp64-intin-long-contract-
//! 2026-09-21.md`).
//!
//! The rule the kernel implements — one rule, not two:
//!
//! > A value wider than a WORD lies **natively in place** at `&int_x[n]`
//! > (little-endian, WORD-aligned, unaligned for its own width). A `LONG` is
//! > 32 bits over two WORDs — `int_x[n]` = bits 0..15, `int_x[n+1]` = bits
//! > 16..31 — which is EmuTOS's own `MAKE_ULONG(int_x[n+1], int_x[n])` order
//! > (`references/emutos/aes/gemsuper.c:143,150`, `include/aesdefs.h:190-191,
//! > 220-221`) and, on a little-endian machine, also what a native 32-bit
//! > store at `&int_x[n]` leaves. A pointer is the caller's pointer width
//! > (A81, `Aespb::intin_ptr`).
//!
//! What this pins, reader by reader:
//!
//! * `evnt_timer` (24): `int_in[0]` low, `int_in[1]` high
//!   (`tos-api/src/aes/event.rs`, `evnt_timer`) — driven by the harness clock
//!   (one tick per read): a count of `[300, 0]` is over after ~300 reads, a
//!   count of `[0, 1]` needs 65 536. The pristine gemlib `a_evnt_timer.c`
//!   handed the AES `[high, low]` on LP64 (H03); the arithmetic overlay and
//!   every `*(long *)&int_in[n] = v` accessor hand it `[low, high]`.
//! * `evnt_multi` (25): `int_in[14]` low, `int_in[15]` high, same reader.
//! * `wind_get(WF_SCREEN)` (104/17): `int_out[1..2]` and `int_out[3..4]` are
//!   two native 32-bit stores (`Aespb::set_intout_long`, A57), EmuTOS's
//!   `gsx_mret((LONG *)poutwds, (LONG *)(poutwds + 2))`
//!   (`aes/gemwmlib.c:1564`, `gemgsxif.c:125-129`). The test also records
//!   what an **8-byte** reader gets there — gemlib's `aes_intout_ptr(1, void
//!   *)` (`a_wind_get_ptr.c:51`) and cflib's `*((void **)&aes_intout[1])`
//!   (`sdk/cflib/cflib-21/purec.c:166`): the address in the low half and the
//!   length in the high half, i.e. **not** the address. That is the
//!   gemlib/cflib-side mismatch of `TASK/T04.md` class 3; the kernel side is
//!   green.
//!
//! Harness: `aes_reference/harness.rs` (LP64 parameter block, poison, the
//! A57 screen-buffer fixture).

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK, SCREEN_BUFFER_PA};

const OP_EVNT_TIMER: i16 = 24;
const OP_EVNT_MULTI: i16 = 25;
const OP_WIND_GET: i16 = 104;
const WF_SCREEN: i16 = 17;
const MU_TIMER: i16 = 0x0020;

/// The harness clock: `monotonic_millis` advances by one on every read, so
/// the number of reads the AES made while waiting *is* the elapsed time.
fn clock() -> u64 {
    tos_api::host::host().expect("harness host").monotonic_millis()
}

/// Lay a 32-bit LONG over `int_in[word..word+2]` the way every LP64 gemlib
/// accessor does — one native little-endian store at `&int_in[word]`
/// (`*(long *)(aes_intin + n) = v` writes these two WORDs first, then spills
/// the upper half; `(short *)&long` as the array is the same bytes).
fn put_long(pb: &mut Pb, word: usize, v: u32) {
    let b = v.to_le_bytes();
    pb.intin[word] = i16::from_le_bytes([b[0], b[1]]);
    pb.intin[word + 1] = i16::from_le_bytes([b[2], b[3]]);
}

/// The LONG at `&int_out[word]` read the way EmuTOS wrote it: 32 bits.
fn long_at(pb: &Pb, word: usize) -> u32 {
    let a = pb.intout[word].to_le_bytes();
    let b = pb.intout[word + 1].to_le_bytes();
    u32::from_le_bytes([a[0], a[1], b[0], b[1]])
}

/// What `*(void **)&int_out[word]` / `aes_intout_ptr(word, void *)` reads on
/// LP64: eight bytes, four WORDs.
fn u64_at(pb: &Pb, word: usize) -> u64 {
    let mut bytes = [0u8; 8];
    for (i, w) in pb.intout[word..word + 4].iter().enumerate() {
        let b = w.to_le_bytes();
        bytes[2 * i] = b[0];
        bytes[2 * i + 1] = b[1];
    }
    u64::from_le_bytes(bytes)
}

#[test]
fn evnt_timer_reads_int_in_0_as_the_low_word_and_int_in_1_as_the_high_word() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // [300, 0]: the native LE 32-bit form of 300 — what the a_evnt_timer.c
    // overlay (arithmetic) and every `*(long *)&int_in[0] = 300` produce.
    pb.poison();
    put_long(&mut pb, 0, 300);
    assert_eq!((pb.intin[0], pb.intin[1]), (300, 0), "the packing under test");
    let t0 = clock();
    pb.call(OP_EVNT_TIMER, 2, 1, 0, 0);
    let dt = clock() - t0;
    assert_eq!(pb.intout[0], 1, "evnt_timer returns 1");
    assert!(
        (300..1_000).contains(&dt),
        "evnt_timer([300, 0]) waited {dt} clock reads: int_in[0] must be the LOW word (T_LOCOUNT)"
    );

    // [0, 1]: 65 536. If int_in[1] were the low word this would return at
    // once; if it is the high word (EmuTOS T_HICOUNT) it takes 65 536 reads.
    pb.poison();
    put_long(&mut pb, 0, 1 << 16);
    assert_eq!((pb.intin[0], pb.intin[1]), (0, 1), "the packing under test");
    let t0 = clock();
    pb.call(OP_EVNT_TIMER, 2, 1, 0, 0);
    let dt = clock() - t0;
    assert!(
        (65_536..70_000).contains(&dt),
        "evnt_timer([0, 1]) waited {dt} clock reads: int_in[1] must be the HIGH word (T_HICOUNT)"
    );
}

#[test]
fn evnt_multi_reads_int_in_14_as_the_low_word_and_int_in_15_as_the_high_word() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // MU_TIMER only, no mouse rectangles, count = 200 as [200, 0].
    pb.poison();
    for w in pb.intin.iter_mut() {
        *w = 0;
    }
    pb.intin[0] = MU_TIMER;
    put_long(&mut pb, 14, 200);
    assert_eq!((pb.intin[14], pb.intin[15]), (200, 0), "the packing under test");
    let t0 = clock();
    pb.call(OP_EVNT_MULTI, 16, 7, 1, 0);
    let dt = clock() - t0;
    assert_eq!(pb.intout[0], MU_TIMER, "evnt_multi returns the timer event");
    // The multiplexer loop reads the clock more than once per pass (arm,
    // deadline check, pump); the bound only has to exclude 200 << 16.
    assert!(
        (200..10_000).contains(&dt),
        "evnt_multi([.., 200, 0]) waited {dt} clock reads: int_in[14] must be the LOW word (MT_LOCOUNT)"
    );
}

#[test]
fn wf_screen_writes_two_native_32bit_longs_and_an_8_byte_reader_gets_addr_or_len_shl_32() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    pb.poison();
    pb.intin[0] = 0;
    pb.intin[1] = WF_SCREEN;
    pb.call(OP_WIND_GET, 2, 5, 0, 0);
    assert_eq!(pb.intout[0], 1, "wind_get(0, WF_SCREEN) answers TRUE with the A57 buffer");

    // The contract: two 32-bit LONGs, [lo, hi] each, at int_out[1] and [3].
    let addr = long_at(&pb, 1);
    let len = long_at(&pb, 3);
    assert_eq!(addr, SCREEN_BUFFER_PA as u32, "int_out[1..2] = the buffer address as one 32-bit LONG");
    assert_ne!(len, 0, "int_out[3..4] = the buffer length as one 32-bit LONG");
    assert_eq!(pb.intout[1] as u16, (addr & 0xFFFF) as u16, "int_out[1] is the LOW word of the address");
    assert_eq!(pb.intout[2] as u16, (addr >> 16) as u16, "int_out[2] is the HIGH word of the address");
    assert_eq!(pb.intout[3] as u16, (len & 0xFFFF) as u16, "int_out[3] is the LOW word of the length");
    assert_eq!(pb.intout[4] as u16, (len >> 16) as u16, "int_out[4] is the HIGH word of the length");

    // What an LP64 8-byte reader at &int_out[1] gets — gemlib's
    // `aes_intout_ptr(1, void *)` (a_wind_get_ptr.c:51) and cflib's
    // `*((void **)&aes_intout[1])` (purec.c:166): the length in the upper
    // half. Not the address. This is the T04 class-3 mismatch on the
    // binding side; the kernel's two 32-bit stores are the contract.
    let wide = u64_at(&pb, 1);
    assert_eq!(
        wide,
        (addr as u64) | ((len as u64) << 32),
        "an 8-byte read at &int_out[1] spans both LONGs"
    );
    assert_ne!(wide, addr as u64, "…and therefore is NOT the address a `void *` reader expects");
}
