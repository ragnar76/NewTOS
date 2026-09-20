//! A50 — the edges of `rsrc_gaddr` (112) / `rsrc_saddr` (113): what comes
//! back for an unknown `R_*` type, for a NULL slot, for an index past the
//! table, and with nothing loaded.
//!
//! EmuTOS `rs_gaddr` (`aes/gemrslib.c:715-721`) and `rs_saddr` (`:725-742`):
//!
//! ```c
//! *rsaddr = get_addr(rtype, rindex);
//! return (*rsaddr != (void *)-1L);            /* rs_gaddr, :720 */
//!
//! psubstruct = (void **)get_addr(rtype, rindex);
//! if (psubstruct != (void **)-1L)             /* rs_saddr, :733 */
//! {
//!     *psubstruct = rsaddr;
//!     return TRUE;
//! }
//! return FALSE;
//! ```
//!
//! and `get_addr` (`:112-176`) answers `(void *)-1L` in its `default:` arm
//! only (`:170-172`); every defined type computes an address, and
//! `R_STRING`/`R_IMAGEDATA` return whatever the free-string / free-image slot
//! holds (`:159-162`) — for a NULL slot that is `0`, and `0 != -1`, so the
//! return code is `TRUE`.
//!
//! NewTOS follows the `-1` rule (user decision 2026-09-20, `TASK/A50.md`,
//! option (a)) and keeps its index bound as a **named deviation**
//! (`docs/COMPATIBILITY.md`, "`rsrc_gaddr`/`rsrc_saddr` refuse an index past
//! the table"): an index beyond the table's count answers `0`/`0` where EmuTOS
//! computes an address past the table and answers `TRUE`. Nothing loaded is
//! `0`/`0` by the A39 contract (`docs/api/aes/resource-fileselector.md` §1,
//! callout item 4).
//!
//! Measured on the reference TT (`measurement-2026-09-12-aes-r4/`, cases
//! 623–625 of `tosdump/cases/aes-r4-subfunctions.tsv`): `rsrc_gaddr` with
//! type 17 / 99 / −1 answers `addr_out[0] = 0xFFFFFFFF`, `int_out[0] = 0`.
//!
//! Spec: `docs/api/aes/resource-fileselector.md` §2 and "Error conditions".
//!
//! The blob is hand-built and registered through
//! `tos_api::aes::resource::relocate_and_register` — the in-place EL1 path —
//! so the tests need no file system. The calls go through the real AES
//! dispatcher with a zeroed `AESGLOBAL`, which the A39 selector resolves to
//! the process's latest unnamed load (`resource.rs`, `selected`).

use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::bdos::process::Abi;

// `rsrc_gaddr` selectors (`aes/resource.rs`, EmuTOS `aesdefs.h:578-594`).
const R_TREE: i16 = 0;
const R_OBJECT: i16 = 1;
const R_STRING: i16 = 5;
const R_IMAGEDATA: i16 = 6;
const R_FRSTR: i16 = 15;
const R_FRIMG: i16 = 16;

/// LP64 `Object` stride.
const OBJECT_SIZE: usize = 32;
/// Size of the NRSC v5/v6 header.
const HEADER_SIZE: usize = 68;

/// The AES resource table is one process-wide global, so the tests in this
/// binary must not relocate concurrently.
fn aes() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
}

/// A writable, 8-byte-aligned copy of `src`. The `Vec` must outlive every use
/// of the returned pointer, hence it is handed back to the caller.
fn blob(src: &[u8]) -> (Vec<u64>, *mut u8) {
    let mut v = vec![0u64; src.len().div_ceil(8) + 1];
    let p = v.as_mut_ptr() as *mut u8;
    // SAFETY: `v` holds at least `src.len()` bytes.
    unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), p, src.len()) };
    (v, p)
}

/// One AES call through the real dispatcher (LP64), with a zeroed
/// `AESGLOBAL` → (`int_out[0]`, `addr_out[0]`). The ILP32 width of the same
/// answer is the dispatcher's `Abi::write_ptr` truncation to 4 bytes
/// (`tos-api/src/bdos/process.rs`), which turns `u64::MAX` into the
/// `0xFFFFFFFF` the reference TT recorded; an ILP32 AESPB carries 4-byte array
/// pointers and cannot name this harness's stack (above 4 GiB on macOS), so
/// that width is not driven here.
fn call(opcode: i16, intin: [i16; 4], addrin0: u64) -> (i16, u64) {
    let mut control = [opcode, 2, 1, 1, 0];
    let mut global = [0i16; 16];
    let mut intin = intin;
    let mut intout = [0i16; 8];
    let mut addrin = [addrin0, 0];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    // SAFETY: a well-formed LP64 parameter block, alive for the call.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    (intout[0], addrout[0])
}

/// `rsrc_gaddr(rtype, index)` (LP64) → (return code, address).
fn gaddr(rtype: i16, index: i16) -> (i16, u64) {
    call(112, [rtype, index, 0, 0], 0)
}

/// `rsrc_saddr(rtype, index, addr)` (LP64) → return code.
fn saddr(rtype: i16, index: i16, addr: u64) -> i16 {
    call(113, [rtype, index, 0, 0], addr).0
}

/// `rsrc_free()` — so one test's loaded resource is not another's.
fn rsrc_free() -> i16 {
    call(111, [0; 4], 0).0
}

/// Frees whatever earlier test left registered (a test that failed before
/// its own `rsrc_free` leaves its entry behind; the in-place path's free
/// touches only the table, never the blob), so a test that needs "nothing
/// loaded" starts from it. Bounded by the table's size.
fn drain() {
    for _ in 0..16 {
        if rsrc_free() == 0 {
            return;
        }
    }
    panic!("the resource table did not drain");
}

/// Reads the 8-byte word at `addr`.
fn rd64(addr: u64) -> u64 {
    // SAFETY: every caller passes an address inside a live blob.
    unsafe { (addr as *const u64).read_unaligned() }
}

// ── the blob ──────────────────────────────────────────────────────────────

const S_TRINDEX: usize = HEADER_SIZE; // 68 (4-byte root index, v5)
const S_OBJECTS: usize = 72;
const S_N_OBJECTS: usize = 1;
const S_FRSTR: usize = S_OBJECTS + S_N_OBJECTS * OBJECT_SIZE; // 104
const S_N_FRSTR: usize = 2;
const S_FRIMG: usize = S_FRSTR + S_N_FRSTR * 8; // 120
const S_N_FRIMG: usize = 1;
const S_STRINGS: usize = S_FRIMG + S_N_FRIMG * 8; // 128
const S_TOTAL: usize = 192;

/// Object base type `G_BOX` (`obdefs.h`).
const G_BOX: i16 = 20;
/// `LASTOB` object flag.
const LASTOB: i16 = 0x0020;

/// A hand-built NRSC v5 (LP64) blob with one tree of one `G_BOX`, a
/// free-string table of **two** slots — `frstr[0]` names a string,
/// `frstr[1]` is **NULL** — and a free-image table of one NULL slot. No
/// TEDINFO/ICONBLK/BITBLK arrays (counts 0). The NULL slots survive the
/// relocation unchanged (`fix_nptrs64`, `resource.rs`: a 0 word is left 0).
fn synthetic() -> Vec<u8> {
    let mut b = vec![0u8; S_TOTAL];
    let put32 = |b: &mut Vec<u8>, off: usize, v: u32| b[off..off + 4].copy_from_slice(&v.to_le_bytes());
    let put64 = |b: &mut Vec<u8>, off: usize, v: u64| b[off..off + 8].copy_from_slice(&v.to_le_bytes());
    let put16 = |b: &mut Vec<u8>, off: usize, v: i16| b[off..off + 2].copy_from_slice(&v.to_le_bytes());

    b[0..4].copy_from_slice(b"NRSC");
    put32(&mut b, 4, 5); // version 5 = LP64, 68-byte header
    put32(&mut b, 8, 1); // n_trees
    put32(&mut b, 12, S_N_OBJECTS as u32);
    put32(&mut b, 16, S_TRINDEX as u32);
    put32(&mut b, 20, S_OBJECTS as u32);
    put32(&mut b, 24, S_TOTAL as u32);
    put32(&mut b, 28, S_FRSTR as u32);
    put32(&mut b, 32, S_N_FRSTR as u32);
    put32(&mut b, 36, S_FRIMG as u32);
    put32(&mut b, 40, S_N_FRIMG as u32);
    put32(&mut b, 44, S_STRINGS as u32); // off_ted (0 entries)
    put32(&mut b, 48, 0); // n_ted
    put32(&mut b, 52, S_STRINGS as u32); // off_ib (0 entries)
    put32(&mut b, 56, 0); // n_ib
    put32(&mut b, 60, S_STRINGS as u32); // off_bb (0 entries)
    put32(&mut b, 64, 0); // n_bb

    // tree index: tree 0's root is object 0.
    put32(&mut b, S_TRINDEX, 0);

    // One G_BOX root, LASTOB.
    let o = S_OBJECTS;
    put16(&mut b, o, -1); // ob_next
    put16(&mut b, o + 2, -1); // ob_head
    put16(&mut b, o + 4, -1); // ob_tail
    put16(&mut b, o + 6, G_BOX);
    put16(&mut b, o + 8, LASTOB);
    put64(&mut b, o + 16, 0x0001_1100); // literal colour word
    put16(&mut b, o + 28, 100); // ob_width
    put16(&mut b, o + 30, 50); // ob_height

    // frstr[0] → "abc"; frstr[1] = NULL; frimg[0] = NULL.
    b[S_STRINGS..S_STRINGS + 4].copy_from_slice(b"abc\0");
    put64(&mut b, S_FRSTR, S_STRINGS as u64);
    put64(&mut b, S_FRSTR + 8, 0);
    put64(&mut b, S_FRIMG, 0);
    b
}

/// Registers a fresh copy of the blob; the returned `Vec` keeps it alive.
fn load() -> (Vec<u64>, u64) {
    let src = synthetic();
    let (v, p) = blob(&src);
    // SAFETY: `p` is a writable copy of the blob built above, `src.len()` bytes.
    assert!(
        unsafe { tos_api::aes::resource::relocate_and_register(p, src.len()) },
        "the synthetic v5 blob must load"
    );
    (v, p as u64)
}

// ── (a) unknown type → -1 at the caller's width, ret 0 ────────────────────

/// `get_addr`'s `default:` arm is `return (void *)-1L` (`gemrslib.c:170-172`);
/// `rs_gaddr` stores it and returns `(-1 != -1)` = 0 (`:719-720`). The arm
/// never touches `rs_hdr`, so the answer is the same with and without a
/// loaded resource. Cases 623 (17), 624 (99), 625 (−1 ⇒ 65535 as UWORD).
#[test]
fn an_unknown_type_answers_minus_one_and_false() {
    let _g = aes();
    drain();
    for t in [17i16, 99, -1] {
        assert_eq!(gaddr(t, 0), (0, u64::MAX), "rsrc_gaddr(type {t}) with nothing loaded");
    }
    let (_v, _base) = load();
    for t in [17i16, 99, -1] {
        assert_eq!(gaddr(t, 0), (0, u64::MAX), "rsrc_gaddr(type {t}) with a resource loaded");
    }
    assert_eq!(rsrc_free(), 1);
}

// ── (b) a NULL slot → address 0, ret 1 ────────────────────────────────────

/// `R_STRING` dereferences the free-string slot (`gemrslib.c:159-160`); a
/// NULL slot yields `0`, and `rs_gaddr`'s test is against `-1`, not `0`
/// (`:720`) — so the return code is **TRUE** with `addr_out[0] = 0`.
/// Likewise `R_IMAGEDATA` (`:161-162`).
#[test]
fn a_null_slot_answers_address_zero_and_true() {
    let _g = aes();
    let (_v, base) = load();
    // Precondition: the neighbouring slot resolves, so the table is live.
    let (ok, s0) = gaddr(R_STRING, 0);
    assert_eq!(ok, 1);
    assert_eq!(s0, base + S_STRINGS as u64, "frstr[0] was relocated onto the blob");
    // The NULL slots.
    assert_eq!(gaddr(R_STRING, 1), (1, 0), "R_STRING on a NULL slot: TRUE, address 0");
    assert_eq!(gaddr(R_IMAGEDATA, 0), (1, 0), "R_IMAGEDATA on a NULL slot: TRUE, address 0");
    // The *slot addresses* (R_FRSTR / R_FRIMG) are unaffected by what they hold.
    assert_eq!(gaddr(R_FRSTR, 1), (1, base + S_FRSTR as u64 + 8));
    assert_eq!(gaddr(R_FRIMG, 0), (1, base + S_FRIMG as u64));
    assert_eq!(rsrc_free(), 1);
}

// ── (c) an index past the table → 0/0 (the named deviation) ───────────────

/// EmuTOS computes `base + size * index` unchecked (`get_addr`, `:102-106`)
/// and answers TRUE with a pointer past the table (case 626 of the r4 table,
/// `R_OBJECT` index 1000). NewTOS answers `0`/`0` — the named deviation in
/// `docs/COMPATIBILITY.md`; this pins it so it cannot drift either way.
#[test]
fn an_index_past_the_table_answers_zero_and_false() {
    let _g = aes();
    let (_v, base) = load();
    assert_eq!(gaddr(R_OBJECT, 0).0, 1, "precondition: object 0 resolves");
    assert_eq!(gaddr(R_OBJECT, S_N_OBJECTS as i16), (0, 0), "one past the OBJECT array");
    assert_eq!(gaddr(R_OBJECT, 1000), (0, 0), "case 626's index");
    assert_eq!(gaddr(R_TREE, 1), (0, 0), "one past the tree table");
    assert_eq!(gaddr(R_STRING, S_N_FRSTR as i16), (0, 0), "one past the free-string table");
    assert_eq!(gaddr(R_FRSTR, S_N_FRSTR as i16), (0, 0));
    // An absent table (count 0) is the same bound at index 0.
    assert_eq!(gaddr(2 /* R_TEDINFO */, 0), (0, 0), "a resource without TEDINFOs");
    // And a negative index is a large UWORD, past every table.
    assert_eq!(gaddr(R_OBJECT, -1), (0, 0));
    // The store side refuses the same slots and writes nothing.
    assert_eq!(saddr(R_OBJECT, S_N_OBJECTS as i16, 0x1234), 0);
    assert_eq!(saddr(R_FRSTR, S_N_FRSTR as i16, 0x1234), 0);
    let _ = base;
    assert_eq!(rsrc_free(), 1);
}

// ── (d) nothing loaded → 0/0 (A39 contract) ───────────────────────────────

/// With no blob the caller's words name, every defined type answers `0`/`0`
/// (spec §1 callout item 4: EmuTOS would parse the header behind `ap_rscmem`,
/// NewTOS resolves nothing it did not load). Not the `-1` of an unknown type.
#[test]
fn nothing_loaded_answers_zero_and_false_for_every_defined_type() {
    let _g = aes();
    drain();
    for t in 0..=16i16 {
        assert_eq!(gaddr(t, 0), (0, 0), "rsrc_gaddr(type {t}, 0) with nothing loaded");
    }
    assert_eq!(saddr(R_FRSTR, 0, 0x1234), 0, "rsrc_saddr with nothing loaded");
    assert_eq!(rsrc_free(), 0, "and nothing to free");
}

// ── (e) rsrc_saddr on an unknown type → FALSE, nothing written ────────────

/// `rs_saddr` (`:733-741`): `get_addr` answered `-1L`, so the store is
/// skipped and the return is FALSE. The blob is untouched.
#[test]
fn saddr_on_an_unknown_type_answers_false_and_stores_nothing() {
    let _g = aes();
    let (_v, base) = load();
    let before = (0..S_TOTAL).map(|i| rd64(base + i as u64) & 0xFF).collect::<Vec<_>>();
    for t in [17i16, 99, -1] {
        assert_eq!(saddr(t, 0, 0xDEAD_BEEF), 0, "rsrc_saddr(type {t})");
    }
    let after = (0..S_TOTAL).map(|i| rd64(base + i as u64) & 0xFF).collect::<Vec<_>>();
    assert_eq!(before, after, "an unknown type must not write into the blob");
    // Contrast: a resolvable slot is written through and answers TRUE.
    assert_eq!(saddr(R_FRSTR, 1, 0x1234_5678), 1, "rsrc_saddr(R_FRSTR, 1) stores");
    assert_eq!(rd64(base + S_FRSTR as u64 + 8), 0x1234_5678, "frstr[1] now holds the pointer");
    assert_eq!(gaddr(R_STRING, 1), (1, 0x1234_5678), "and R_STRING reads it back");
    assert_eq!(rsrc_free(), 1);
}

// ── (f) rsrc_saddr through a NULL slot ────────────────────────────────────

/// `R_STRING` on a NULL slot resolves to address 0 (TRUE for `rsrc_gaddr`);
/// `rs_saddr` would then store *through* address 0 and answer TRUE
/// (`:735-738`). NewTOS has no writable address 0 on either side of the
/// EL0/EL1 boundary, so the store is refused: FALSE, nothing written — the
/// second half of the named deviation in `docs/COMPATIBILITY.md`.
#[test]
fn saddr_through_a_null_slot_answers_false_and_stores_nothing() {
    let _g = aes();
    let (_v, base) = load();
    assert_eq!(gaddr(R_STRING, 1), (1, 0), "precondition: the slot is NULL");
    assert_eq!(saddr(R_STRING, 1, 0x1234), 0, "no store through address 0");
    assert_eq!(saddr(R_IMAGEDATA, 0, 0x1234), 0);
    assert_eq!(rd64(base + S_FRSTR as u64 + 8), 0, "frstr[1] is still NULL");
    assert_eq!(rd64(base + S_FRIMG as u64), 0, "frimg[0] is still NULL");
    assert_eq!(rsrc_free(), 1);
}
