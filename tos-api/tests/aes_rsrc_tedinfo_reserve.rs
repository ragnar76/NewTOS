//! A37, second half — the resource **file** reserves the declared
//! `te_txtlen` behind `te_ptext`, so an editable field has the typing room it
//! advertises and typing stays inside it.
//!
//! This is the other half of EmuTOS' division of labour, and it is the half
//! that decides whether a field can be typed into at all:
//!
//! * **`erd` reserves.** EmuTOS' resource compiler gives every editable field
//!   its full `te_txtlen` bytes of `rs_te_text[]` (`tools/erd.c:3006-3007`,
//!   `te_ptext_len += get_short(&ted->te_txtlen)`), and `gem_rsc_init`
//!   (`aes/gem_rsc.c`) hands that array to the AES **without** recomputing the
//!   length words. `gem.rsc` `ted[0]` — the file selector's path field — keeps
//!   `te_txtlen` 39 over an empty text, i.e. 38 characters of typing room.
//! * **`rs_readit` recomputes.** Only a resource loaded from disk goes through
//!   `fix_tedinfo_std` (`aes/gemrslib.c:655-669`), which sets
//!   `te_txtlen = strlen(te_ptext) + 1`. That is A37's first half and it is
//!   already in `relocate_and_register`.
//!
//! NewTOS embeds `gem.rsc` converted by `tools/mkrsc.py` and relocates it with
//! [`relocate_blob`] — the `gem_rsc_init` path, no recompute. So the file's
//! reservation is the *only* thing between `objc_edit` and the next string in
//! the blob: `ins_char` writes up to index `te_txtlen - 1`
//! (`aes/object.rs`, EmuTOS `gemobed.c:117`).
//!
//! Measured before the tool fix (audit B5,
//! `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md:228-271`): `ted[0]`
//! had a **1-byte** buffer against `te_txtlen` 39, with its own template
//! string starting at the very next byte — 38 bytes of overflow, into the
//! `"______…"` template the same field draws with.
//!
//! Spec: `docs/api/aes/resource-fileselector.md`.

use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::bdos::process::Abi;

/// The AES's own GEM.RSC, as `tos-api/src/aes/fsel.rs` embeds it.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

/// `FSDIRECT`, the file selector's editable path field (EmuTOS
/// `aes/gem_rsc.h:17`) — `tedinfo[0]`, the audit's worst case.
const FSDIRECT: usize = 3;

/// LP64 `Object` (32 bytes): `ob_spec` is the 8-byte field at offset 16.
const OBJ_SIZE: usize = 32;
const OB_SPEC: usize = 16;

/// LP64 `TedInfo`: three 8-byte pointers, then eight WORDs.
const TE_PTEXT: usize = 0;
const TE_PTMPLT: usize = 8;
const TE_TXTLEN: usize = 36;
const TE_TMPLEN: usize = 38;

/// `objc_edit` kinds (EmuTOS `EDINIT`/`EDCHAR`).
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;

/// `objc_edit` and the AES scratch it uses are process-wide, so the tests in
/// this binary must not edit concurrently.
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

/// Relocate a fresh copy of the embedded blob the way `fsel.rs` does, and
/// return `(backing store, base pointer, FSELECTR tree)`.
fn embedded() -> (Vec<u64>, *mut u8, u64) {
    let (v, p) = blob(RSC);
    // SAFETY: `p` is a writable copy of a valid NRSC blob, `RSC.len()` bytes.
    assert!(
        unsafe { tos_api::aes::resource::relocate_blob(p, RSC.len()) },
        "the embedded gem.rsc must relocate"
    );
    // SAFETY: same blob, tree 0 exists (FSELECTR).
    let tree = unsafe { tos_api::aes::resource::tree_root(p, 0) } as u64;
    assert!(tree != 0, "tree 0 must resolve");
    (v, p, tree)
}

/// The relocated `TedInfo` address of object `idx` (its `ob_spec`).
fn ted_of(tree: u64, idx: usize) -> u64 {
    // SAFETY: `idx` is a valid object of the relocated tree.
    unsafe { ((tree as usize + idx * OBJ_SIZE + OB_SPEC) as *const u64).read_unaligned() }
}

fn rd64(addr: u64) -> u64 {
    // SAFETY: `addr` is inside the live blob.
    unsafe { (addr as *const u64).read_unaligned() }
}

fn rd16(addr: u64) -> i16 {
    // SAFETY: `addr` is inside the live blob.
    unsafe { (addr as *const i16).read_unaligned() }
}

/// `strlen` of a relocated C string.
fn cstrlen(p: u64) -> usize {
    let mut n = 0usize;
    // SAFETY: a relocated, NUL-terminated blob string.
    unsafe {
        while *(p as *const u8).add(n) != 0 {
            n += 1;
        }
    }
    n
}

/// `objc_edit(tree, obj, in_char, idx, kind)` (opcode 46) through the real
/// dispatcher: `control = {46, 4, 2, 1, 0}`. Returns the new cursor index.
fn objc_edit(tree: u64, obj: i16, in_char: i16, idx: i16, kind: i16) -> i16 {
    let mut control = [46i16, 4, 2, 1, 0];
    let mut global = [0i16; 16];
    let mut intin = [obj, in_char, idx, kind];
    let mut intout = [0i16; 8];
    let mut addrin = [tree, 0];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    // SAFETY: a well-formed AES parameter block.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    assert_eq!(intout[0], 1, "objc_edit must return 1");
    intout[1]
}

/// The blob's declared size (`total_size`, u32 at header offset 24).
fn total_size() -> usize {
    // The NRSC header sits at the front (v1–v12) or behind the 40-byte native
    // `RSHDR` preamble (v13+, `resource.rs` `RSHDR_PAD`); `total_size` is its
    // sixth `u32`.
    let off = if &RSC[..4] == b"NRSC" { 0 } else { 40 };
    assert_eq!(&RSC[off..off + 4], b"NRSC", "an NRSC blob");
    u32::from_le_bytes(RSC[off + 24..off + 28].try_into().unwrap()) as usize
}

// ── the reservation, in the file ──────────────────────────────────────────

/// The counter-check the fix exists for: a field whose resource text is
/// **empty** must still have room to type.
///
/// `gem.rsc` `ted[0]` declares `te_txtlen` 39 over an empty text — 38
/// characters. The embedded path does not recompute the word (EmuTOS
/// `gem_rsc_init`), so the room is real **iff** the file reserves the bytes.
/// Before the tool fix `te_ptext` was one byte long and the very next byte was
/// the field's own template.
#[test]
fn an_empty_selector_field_has_the_typing_room_it_declares() {
    let _g = aes();
    let (_v, p, tree) = embedded();
    let base = p as u64;

    let ted = ted_of(tree, FSDIRECT);
    let ptext = rd64(ted + TE_PTEXT as u64);
    let ptmplt = rd64(ted + TE_PTMPLT as u64);
    let txtlen = rd16(ted + TE_TXTLEN as u64) as usize;
    let tmplen = rd16(ted + TE_TMPLEN as u64) as usize;

    assert_eq!(cstrlen(ptext), 0, "FSDIRECT's resource text is empty in gem.rsc");
    assert_eq!(txtlen, 39, "gem.rsc ted[0] declares te_txtlen 39");
    assert!(
        txtlen > 1,
        "an editable field with no room to type: te_txtlen {txtlen}"
    );

    // The reservation itself, measured the way the audit measured `gem.rsc`:
    // the distance from the text to the next string in the blob.
    assert!(
        ptmplt > ptext,
        "gem.rsc places the template behind the text"
    );
    assert!(
        (ptmplt - ptext) as usize >= txtlen,
        "the blob reserves only {} bytes behind te_ptext (blob offset {}) but \
         te_txtlen is {txtlen} — ins_char writes up to index {}",
        ptmplt - ptext,
        ptext - base,
        txtlen - 1
    );
    // And the reserved bytes are NUL, so the buffer is a valid C string
    // whatever is written into it.
    // SAFETY: inside the reservation just checked.
    let reserved = unsafe { std::slice::from_raw_parts(ptext as *const u8, txtlen) };
    assert!(reserved.iter().all(|&b| b == 0), "the reservation must be zeroed");

    // The template is read-only and its length word *is* its string length
    // (`include/obdefs.h:182`), so it needs no capacity beyond that.
    assert_eq!(
        tmplen,
        cstrlen(ptmplt) + 1,
        "te_tmplen is a string length, not a buffer size"
    );
}

// ── the write itself ──────────────────────────────────────────────────────

/// Type the field full — `te_txtlen - 1` characters and more — and show that
/// not one byte of the blob outside that field's own buffer moves.
///
/// This is the write the audit named. `FSDIRECT`'s template string used to
/// start one byte behind its text, so the first 38 characters overwrote the
/// `"______…"` the same field draws with.
#[test]
fn typing_a_selector_field_full_stays_inside_its_own_buffer() {
    let _g = aes();
    let (_v, p, tree) = embedded();
    let base = p as u64;
    let total = total_size();

    let ted = ted_of(tree, FSDIRECT);
    let ptext = rd64(ted + TE_PTEXT as u64);
    let ptmplt = rd64(ted + TE_PTMPLT as u64);
    let txtlen = rd16(ted + TE_TXTLEN as u64) as usize;
    let text_off = (ptext - base) as usize;
    let tmplt_off = (ptmplt - base) as usize;
    let tmplt_len = cstrlen(ptmplt) + 1;

    // SAFETY: `p` addresses `total` live bytes.
    let before = unsafe { std::slice::from_raw_parts(p, total) }.to_vec();

    // EDINIT, then more characters than the field can hold. 'A' passes the
    // field's 'P' (path) validation, so every one of them is a real insert.
    let mut idx = objc_edit(tree, FSDIRECT as i16, 0, 0, ED_INIT);
    for _ in 0..64 {
        idx = objc_edit(tree, FSDIRECT as i16, i16::from(b'A'), idx, ED_CHAR);
    }

    // SAFETY: same live span.
    let after = unsafe { std::slice::from_raw_parts(p, total) };

    // The damage the audit named, stated on its own.
    assert_eq!(
        &before[tmplt_off..tmplt_off + tmplt_len],
        &after[tmplt_off..tmplt_off + tmplt_len],
        "typing into FSDIRECT overwrote its own te_ptmplt template at blob \
         offset {tmplt_off}"
    );

    // …and the general statement: nothing outside the declared buffer.
    let changed: Vec<usize> = (0..total)
        .filter(|&i| before[i] != after[i])
        .filter(|&i| !(text_off..text_off + txtlen).contains(&i))
        .collect();
    assert!(
        changed.is_empty(),
        "editing FSDIRECT (te_ptext at blob offset {text_off}, te_txtlen \
         {txtlen}) changed blob bytes outside its buffer: {changed:?}"
    );

    // Not vacuous: the field really did fill up to its declared capacity.
    let typed = cstrlen(ptext);
    assert_eq!(
        typed,
        txtlen - 1,
        "the field should hold te_txtlen-1 characters after typing it full"
    );
    assert_eq!(idx as usize, typed, "the cursor sits behind the last character");
}
