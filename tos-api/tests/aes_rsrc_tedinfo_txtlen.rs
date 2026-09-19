//! `rsrc_load` recomputes the TEDINFO length words after relocation —
//! EmuTOS `fix_tedinfo_std` (`aes/gemrslib.c:655-669`), called from
//! `rs_readit` (`gemrslib.c:798`):
//!
//! ```c
//! if (fix_ptr(R_TEPTEXT,  ii)) ted->te_txtlen = strlen(ted->te_ptext)  + 1;
//! if (fix_ptr(R_TEPTMPLT, ii)) ted->te_tmplen = strlen(ted->te_ptmplt) + 1;
//! ```
//!
//! Spec: `docs/api/aes/resource-fileselector.md` §1 `rsrc_load (110)`.
//!
//! The regression this guards (audit B5,
//! `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md:228-271`): NewTOS kept
//! the file's `te_txtlen` while `tools/mkrsc.py` `intern_str` copies only up to
//! the first NUL, so the text buffer in the converted blob was *shorter* than
//! `te_txtlen`. Every writer bounds itself by `te_txtlen`, not by the buffer —
//! `ins_char` (`aes/object.rs`, EmuTOS `gemobed.c:117`) writes up to index
//! `te_txtlen - 1` — so typing into such a field wrote past the buffer into the
//! strings that follow it in the blob. Measured on the shipped
//! `src/aes/gem_fsel.nrsc`: 11 of 13 TEDINFOs affected, worst case a 1-byte
//! buffer against `te_txtlen` 39.
//!
//! The tests drive the real relocation entry point and the real `objc_edit`
//! trap, and check the blob bytes themselves — not just the length word.

use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::bdos::process::Abi;

/// The AES's own GEM.RSC, a genuine `NRSC` blob: the file the audit measured.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

// `rsrc_gaddr` selectors (`aes/resource.rs`, EmuTOS `gemrslib.c:112-176`).
const R_TREE: i16 = 0;
const R_TEDINFO: i16 = 2;

/// Byte offsets inside the LP64 `TedInfo` (three 8-byte pointers, then eight
/// WORDs; `te_txtlen` is the seventh).
const TE_PTEXT: u64 = 0;
const TE_PTMPLT: u64 = 8;
const TE_PVALID: u64 = 16;
const TE_TXTLEN: u64 = 36;
const TE_TMPLEN: u64 = 38;

/// `objc_edit` kinds (EmuTOS `EDINIT`/`EDCHAR`).
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;

/// `FSDIRECT`, the file selector's editable path field (EmuTOS
/// `aes/gem_rsc.h:17`). Its TEDINFO is `tedinfo[0]` — the audit's worst case.
const FSDIRECT: i16 = 3;

/// The AES resource descriptor is one process-wide global, so the tests in
/// this binary must not relocate concurrently.
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

/// `objc_edit(tree, obj, in_char, idx, kind)` (opcode 46) through the real
/// dispatcher: `control = {46, 4, 2, 1, 0}` — four input WORDs, two output
/// WORDs (`RET_CODE` and the new cursor index), one input address.
/// Returns the new cursor index, `int_out[1]`.
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

/// `rsrc_gaddr(rtype, index)` → (return code, address).
fn gaddr(rtype: i16, index: i16) -> (i16, u64) {
    let mut control = [112i16, 2, 1, 0, 0];
    let mut global = [0i16; 16];
    let mut intin = [rtype, index, 0, 0];
    let mut intout = [0i16; 8];
    let mut addrin = [0u64; 2];
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
    (intout[0], addrout[0])
}

/// Read a `u64` field of a relocated record.
fn rd64(addr: u64) -> u64 {
    // SAFETY: `addr` is inside the live blob.
    unsafe { (addr as *const u64).read_unaligned() }
}

/// Read an `i16` field of a relocated record.
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

/// The `u32` at byte `off` of the blob's NRSC header, which sits at the front
/// (v1–v12) or behind the 40-byte native `RSHDR` preamble (v13+, `resource.rs`
/// `RSHDR_PAD`): 24 `total_size`, 48 `n_ted`.
fn hdr_u32(src: &[u8], off: usize) -> usize {
    let base = if &src[..4] == b"NRSC" { 0 } else { 40 };
    assert_eq!(&src[base..base + 4], b"NRSC", "an NRSC blob");
    u32::from_le_bytes(src[base + off..base + off + 4].try_into().unwrap()) as usize
}

/// Number of TEDINFO records the blob declares (`n_ted` at header offset 48 of
/// the 68-byte v5 header).
fn n_ted(src: &[u8]) -> usize {
    hdr_u32(src, 48)
}

// ── the length words themselves ───────────────────────────────────────────

/// EmuTOS `aes/gemrslib.c:660-668` — every relocated TEDINFO's `te_txtlen` and
/// `te_tmplen` is `strlen + 1` of the string it points at.
#[test]
fn tedinfo_lengths_are_recomputed_from_the_strings() {
    let _g = aes();
    let (_v, p) = blob(RSC);
    // SAFETY: `p` is a writable copy of a valid NRSC blob, `RSC.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, RSC.len()) });

    for i in 0..n_ted(RSC) {
        let (ret, ted) = gaddr(R_TEDINFO, i as i16);
        assert_eq!(ret, 1, "rsrc_gaddr(R_TEDINFO, {i}) must resolve");
        let ptext = rd64(ted + TE_PTEXT);
        assert_eq!(
            rd16(ted + TE_TXTLEN) as usize,
            cstrlen(ptext) + 1,
            "tedinfo[{i}].te_txtlen was not recomputed; ins_char writes \
             te_txtlen bytes into a strlen+1-byte buffer"
        );
        let ptmplt = rd64(ted + TE_PTMPLT);
        assert_eq!(
            rd16(ted + TE_TMPLEN) as usize,
            cstrlen(ptmplt) + 1,
            "tedinfo[{i}].te_tmplen was not recomputed"
        );
    }
}

/// The invariant the recompute exists to restore: a TEDINFO may not claim more
/// text bytes than the blob actually reserves behind `te_ptext`.
///
/// "Reserved" is bounded above by the distance to the next string the blob
/// already uses for something else. The bound is computed from the TEDINFO
/// string pointers only — free strings and `ob_spec` labels may lie closer
/// still, so this is a conservative bound, and it is exactly the one the audit
/// measured: `tedinfo[0]`'s template starts one byte behind its text.
#[test]
fn no_tedinfo_claims_more_bytes_than_the_blob_reserves() {
    let _g = aes();
    let (_v, p) = blob(RSC);
    // SAFETY: `p` is a writable copy of a valid NRSC blob, `RSC.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, RSC.len()) });
    let base = p as u64;
    let total = hdr_u32(RSC, 24) as u64; // total_size

    // Every string address the TEDINFOs point at.
    let mut used: Vec<u64> = Vec::new();
    for i in 0..n_ted(RSC) {
        let (ret, ted) = gaddr(R_TEDINFO, i as i16);
        assert_eq!(ret, 1, "rsrc_gaddr(R_TEDINFO, {i}) must resolve");
        for off in [TE_PTEXT, TE_PTMPLT, TE_PVALID] {
            let q = rd64(ted + off);
            if q != 0 {
                used.push(q);
            }
        }
    }
    used.push(base + total); // the blob's end bounds the last string
    used.sort_unstable();

    for i in 0..n_ted(RSC) {
        let (_, ted) = gaddr(R_TEDINFO, i as i16);
        let ptext = rd64(ted + TE_PTEXT);
        if ptext == 0 {
            continue;
        }
        let txtlen = rd16(ted + TE_TXTLEN) as u64;
        let next = *used.iter().find(|&&q| q > ptext).expect("blob end is in the set");
        assert!(
            txtlen <= next - ptext,
            "tedinfo[{i}]: te_txtlen {txtlen} exceeds the {} bytes the blob \
             reserves behind te_ptext (the next string starts there)",
            next - ptext
        );
    }
}

// ── the overflow itself ───────────────────────────────────────────────────

/// The write, not the length word: typing into an editable field of a loaded
/// resource must not change a single byte of the blob outside that field's own
/// text buffer.
///
/// `FSDIRECT` is the audit's worst case — `tedinfo[0]`, a 1-byte buffer with
/// `te_txtlen` 39 in the file, whose `te_ptmplt` string starts at the very next
/// byte. Before the fix, 38 bytes of typing walked straight into that template.
#[test]
fn typing_into_a_loaded_field_touches_nothing_outside_its_text_buffer() {
    let _g = aes();
    let (_v, p) = blob(RSC);
    // SAFETY: `p` is a writable copy of a valid NRSC blob, `RSC.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, RSC.len()) });
    let base = p as u64;
    let total = hdr_u32(RSC, 24);

    let (ret, ted) = gaddr(R_TEDINFO, 0);
    assert_eq!(ret, 1);
    let ptext = rd64(ted + TE_PTEXT);
    let text_off = (ptext - base) as usize;
    // The bytes the *blob* reserves, read off the blob and not off `te_txtlen`
    // — anchoring the window to `te_txtlen` would make the test vacuous exactly
    // in the broken state it must catch. `mkrsc.py` interns each string up to
    // its NUL, so the reserved span is `strlen + 1`; the field can be retyped
    // within it but never grown, because `te_txtlen` is now that same value.
    let buf_len = cstrlen(ptext) + 1;
    let tmplt = rd64(ted + TE_PTMPLT);
    let tmplt_off = (tmplt - base) as usize;
    let tmplt_len = cstrlen(tmplt) + 1;

    let (ret, tree) = gaddr(R_TREE, 0);
    assert_eq!(ret, 1, "rsrc_gaddr(R_TREE, 0) must resolve");

    // SAFETY: `p` addresses `total` live bytes.
    let bytes = unsafe { std::slice::from_raw_parts(p, total) };
    let before = bytes.to_vec();

    // EDINIT, then far more characters than any buffer in this blob can hold.
    let mut idx = objc_edit(tree, FSDIRECT, 0, 0, ED_INIT);
    for _ in 0..64 {
        idx = objc_edit(tree, FSDIRECT, i16::from(b'A'), idx, ED_CHAR);
    }

    // SAFETY: same live span.
    let after = unsafe { std::slice::from_raw_parts(p, total) };

    // The damage the audit named, stated on its own: the template string that
    // sits immediately behind the text buffer.
    assert_eq!(
        &before[tmplt_off..tmplt_off + tmplt_len],
        &after[tmplt_off..tmplt_off + tmplt_len],
        "typing into tedinfo[0] overwrote its own te_ptmplt template at blob \
         offset {tmplt_off}"
    );

    // …and the general statement: nothing outside the reserved text buffer.
    let changed: Vec<usize> = (0..total)
        .filter(|&i| before[i] != after[i])
        .filter(|&i| !(text_off..text_off + buf_len).contains(&i))
        .collect();
    assert!(
        changed.is_empty(),
        "editing tedinfo[0] (te_ptext at blob offset {text_off}, {buf_len} \
         reserved bytes) changed blob bytes outside its buffer: {changed:?}"
    );
}
