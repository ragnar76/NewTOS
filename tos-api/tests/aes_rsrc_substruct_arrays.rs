//! A36 — the TEDINFO / ICONBLK / BITBLK sub-structures of a loaded `.RSC`
//! are relocated **array-wise**, over the counts the header declares, not
//! only where an OBJECT happens to point.
//!
//! EmuTOS `rs_readit` (`aes/gemrslib.c:798-805`) walks each array once, by
//! count, and never consults an object:
//!
//! ```c
//! fix_tedinfo_std();                      /* rsh_nted   */
//! ibcnt = rs_hdr->rsh_nib;
//! fix_nptrs(ibcnt, R_IBPMASK);
//! fix_nptrs(ibcnt, R_IBPDATA);
//! fix_nptrs(ibcnt, R_IBPTEXT);
//! fix_nptrs(rs_hdr->rsh_nbb, R_BIPDATA);  /* rsh_nbb    */
//! fix_nptrs(rs_hdr->rsh_nstring, R_FRSTR);
//! fix_nptrs(rs_hdr->rsh_nimages, R_FRIMG);
//! ```
//!
//! `fix_nptrs` (`gemrslib.c:640-646`) is `for (i=0; i<cnt; i++)
//! fix_long(get_addr(type, i))`, and `get_addr` indexes the header's array
//! base (`gemrslib.c:112-176`) — the OBJECT array plays no part. The objects
//! are fixed up separately and later, in `rs_fixit` → `fix_objects`
//! (`gemrslib.c:606-637`, `rs_load` `:846-847`), which only turns each
//! `ob_spec` into a pointer.
//!
//! The regression this guards (audit B4,
//! `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md:192-227`): NewTOS
//! relocated a sub-structure only when some OBJECT referenced it, de-duplicated
//! over the already-processed objects. A record that no object points at kept
//! its raw file offsets. Measured on the shipped `src/aes/gem_fsel.nrsc`: the
//! three BITBLKs are reachable only through the free-image table, so
//! `rsrc_gaddr(R_BITBLK, i)` handed back `bi_pdata` as a small integer — a wild
//! pointer for any app that fetches a free image and draws it.
//!
//! Spec: `docs/api/aes/resource-fileselector.md` §1 `rsrc_load (110)` /
//! §`rsrc_gaddr (112)`.

use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::bdos::process::Abi;

/// The AES's own GEM.RSC, a genuine `NRSC` blob — the file the audit measured.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

// `rsrc_gaddr` selectors (`aes/resource.rs`, EmuTOS `gemrslib.c:112-176`).
const R_TEDINFO: i16 = 2;
const R_ICONBLK: i16 = 3;
const R_BITBLK: i16 = 4;
const R_IMAGEDATA: i16 = 6;

/// LP64 `TedInfo` field offsets (three 8-byte pointers, then eight WORDs).
const TE_PTEXT: u64 = 0;
const TE_PTMPLT: u64 = 8;
const TE_PVALID: u64 = 16;
const TE_TXTLEN: u64 = 36;
const TE_TMPLEN: u64 = 38;

/// LP64 `IconBlk` field offsets (three 8-byte pointers, then twelve WORDs).
const IB_PMASK: u64 = 0;
const IB_PDATA: u64 = 8;
const IB_PTEXT: u64 = 16;

/// LP64 `BitBlk`: one 8-byte pointer, then five WORDs (padded to 24).
const BI_PDATA: u64 = 0;

/// Record strides of the LP64 sub-structures, as `mkrsc.py` emits them.
const TEDINFO_SIZE: usize = 40;
const ICONBLK_SIZE: usize = 48;
const BITBLK_SIZE: usize = 24;
/// LP64 `Object` stride.
const OBJECT_SIZE: usize = 32;

/// NRSC v5 (LP64) header field byte offsets.
const H_TOTAL_SIZE: usize = 24;
const H_N_TED: usize = 48;
const H_N_BB: usize = 64;
/// Size of the v5/v6 header.
const HEADER_SIZE: usize = 68;

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

/// `rsrc_gaddr(rtype, index)` through the real dispatcher → (return code,
/// address).
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
    // SAFETY: a well-formed LP64 parameter block, alive for the call.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    (intout[0], addrout[0])
}

/// `rsrc_free()` — so one test's loaded resource is not another's.
fn rsrc_free() {
    let mut control = [111i16, 0, 1, 0, 0];
    let mut global = [0i16; 16];
    let mut intin = [0i16; 4];
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
    // SAFETY: a well-formed LP64 parameter block, alive for the call.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
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

fn hdr_u32(src: &[u8], off: usize) -> usize {
    u32::from_le_bytes(src[off..off + 4].try_into().unwrap()) as usize
}

/// A relocated pointer must address a byte inside the blob, i.e. it is
/// `base + offset` with `offset` below the declared `total_size` — never the
/// bare offset the file stores.
fn assert_relocated(value: u64, base: u64, total: u64, what: &str) {
    assert!(
        value >= base && value < base + total,
        "{what} = {value} is not a pointer into the blob \
         (base {base}, total_size {total}); the file offset was never rebased"
    );
}

// ── the shipped resource ──────────────────────────────────────────────────

/// EmuTOS `gemrslib.c:803` — `fix_nptrs(rs_hdr->rsh_nbb, R_BIPDATA)`.
///
/// `gem_fsel.nrsc` declares three BITBLKs and contains **no** `G_IMAGE`
/// object: they are reachable only through the free-image table. The
/// object-driven walk therefore never saw them and `bi_pdata` stayed a raw
/// file offset.
#[test]
fn every_bitblk_pdata_is_relocated() {
    let _g = aes();
    let (_v, p) = blob(RSC);
    // SAFETY: `p` is a writable copy of a valid NRSC blob, `RSC.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, RSC.len()) });
    let base = p as u64;
    let total = hdr_u32(RSC, H_TOTAL_SIZE) as u64;
    let n_bb = hdr_u32(RSC, H_N_BB);
    assert!(n_bb > 0, "gem_fsel.nrsc declares BITBLKs");

    for i in 0..n_bb {
        let (ret, bb) = gaddr(R_BITBLK, i as i16);
        assert_eq!(ret, 1, "rsrc_gaddr(R_BITBLK, {i}) must resolve");
        assert_relocated(rd64(bb + BI_PDATA), base, total, &format!("bitblk[{i}].bi_pdata"));
    }
    rsrc_free();
}

/// The failure case the audit named, through the documented API: an
/// application that fetches a **free image** and reads its raster.
/// `R_IMAGEDATA` dereferences the free-image slot, which points at a BITBLK
/// — whose `bi_pdata` must be a pointer, not a small integer.
#[test]
fn a_free_image_yields_a_dereferenceable_raster() {
    let _g = aes();
    let (_v, p) = blob(RSC);
    // SAFETY: `p` is a writable copy of a valid NRSC blob, `RSC.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, RSC.len()) });
    let base = p as u64;
    let total = hdr_u32(RSC, H_TOTAL_SIZE) as u64;

    let (ret, bitblk) = gaddr(R_IMAGEDATA, 0);
    assert_eq!(ret, 1, "rsrc_gaddr(R_IMAGEDATA, 0) must resolve");
    assert_relocated(bitblk, base, total, "frimg[0]");
    assert_relocated(rd64(bitblk + BI_PDATA), base, total, "frimg[0]->bi_pdata");
    rsrc_free();
}

/// EmuTOS `gemrslib.c:655-669` — `fix_tedinfo_std` walks `rsh_nted` records,
/// not the objects that reference them.
#[test]
fn every_tedinfo_pointer_is_relocated() {
    let _g = aes();
    let (_v, p) = blob(RSC);
    // SAFETY: `p` is a writable copy of a valid NRSC blob, `RSC.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, RSC.len()) });
    let base = p as u64;
    let total = hdr_u32(RSC, H_TOTAL_SIZE) as u64;
    let n_ted = hdr_u32(RSC, H_N_TED);
    assert!(n_ted > 0, "gem_fsel.nrsc declares TEDINFOs");

    for i in 0..n_ted {
        let (ret, ted) = gaddr(R_TEDINFO, i as i16);
        assert_eq!(ret, 1, "rsrc_gaddr(R_TEDINFO, {i}) must resolve");
        for (off, name) in [(TE_PTEXT, "te_ptext"), (TE_PTMPLT, "te_ptmplt"), (TE_PVALID, "te_pvalid")] {
            let v = rd64(ted + off);
            if v != 0 {
                assert_relocated(v, base, total, &format!("tedinfo[{i}].{name}"));
            }
        }
    }
    rsrc_free();
}

// ── a resource whose sub-structures no object points at ───────────────────

/// Offsets of the synthetic blob's sections. Every array base is 8-aligned,
/// as `mkrsc.py` emits them and as the LP64 record types require.
const S_TRINDEX: usize = HEADER_SIZE; // 68
const S_OBJECTS: usize = 72;
const S_N_OBJECTS: usize = 3;
const S_TED: usize = S_OBJECTS + S_N_OBJECTS * OBJECT_SIZE; // 168
const S_N_TED: usize = 2;
const S_IB: usize = S_TED + S_N_TED * TEDINFO_SIZE; // 248
const S_BB: usize = S_IB + ICONBLK_SIZE; // 296
const S_STRINGS: usize = S_BB + BITBLK_SIZE; // 320
const S_TOTAL: usize = 512;

/// Object base types used below (`obdefs.h`).
const G_BOX: i16 = 20;
const G_BOXTEXT: i16 = 22;

/// A hand-built NRSC v5 blob in which
///
/// * `tedinfo[0]`, `iconblk[0]` and `bitblk[0]` are referenced by **no**
///   object — EmuTOS relocates them all the same, by count;
/// * `tedinfo[1]` is referenced by **two** objects — it must be relocated
///   exactly once, or its pointers end up `base + base + offset`;
/// * `tedinfo[0]`'s length words are deliberately wrong in the file, so the
///   `strlen + 1` recompute (`gemrslib.c:660-668`) is visible on a record the
///   object walk never reached.
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
    put32(&mut b, H_TOTAL_SIZE, S_TOTAL as u32);
    put32(&mut b, 28, S_STRINGS as u32); // off_frstr (0 entries)
    put32(&mut b, 32, 0); // n_frstr
    put32(&mut b, 36, S_STRINGS as u32); // off_frimg (0 entries)
    put32(&mut b, 40, 0); // n_frimg
    put32(&mut b, 44, S_TED as u32);
    put32(&mut b, H_N_TED, S_N_TED as u32);
    put32(&mut b, 52, S_IB as u32);
    put32(&mut b, 56, 1); // n_ib
    put32(&mut b, 60, S_BB as u32);
    put32(&mut b, H_N_BB, 1); // n_bb

    // tree index: tree 0's root is object 0.
    put32(&mut b, S_TRINDEX, 0);

    // Strings, each NUL-terminated.
    let s_text = S_STRINGS; // "abc"
    let s_tmplt = S_STRINGS + 8; // "__"
    let s_valid = S_STRINGS + 16; // "9"
    let s_icon = S_STRINGS + 24; // "ICON"
    b[s_text..s_text + 4].copy_from_slice(b"abc\0");
    b[s_tmplt..s_tmplt + 3].copy_from_slice(b"__\0");
    b[s_valid..s_valid + 2].copy_from_slice(b"9\0");
    b[s_icon..s_icon + 5].copy_from_slice(b"ICON\0");

    // Objects: a G_BOX root with two G_BOXTEXT children, both pointing at
    // tedinfo[1]. Nothing points at tedinfo[0], iconblk[0] or bitblk[0].
    let obj = |b: &mut Vec<u8>, i: usize, next: i16, head: i16, tail: i16, ty: i16, flags: i16, spec: u64| {
        let o = S_OBJECTS + i * OBJECT_SIZE;
        put16(b, o, next);
        put16(b, o + 2, head);
        put16(b, o + 4, tail);
        put16(b, o + 6, ty);
        put16(b, o + 8, flags);
        put16(b, o + 10, 0); // ob_state
        put64(b, o + 16, spec);
        put16(b, o + 24, 0); // ob_x
        put16(b, o + 26, 0); // ob_y
        put16(b, o + 28, 100); // ob_width
        put16(b, o + 30, 50); // ob_height
    };
    let ted1 = (S_TED + TEDINFO_SIZE) as u64;
    obj(&mut b, 0, -1, 1, 2, G_BOX, 0, 0x0001_1100); // literal colour word
    obj(&mut b, 1, 2, -1, -1, G_BOXTEXT, 0, ted1);
    obj(&mut b, 2, 0, -1, -1, G_BOXTEXT, 0x0020, ted1); // LASTOB

    // tedinfo[0] — unreferenced, and its length words are wrong in the file.
    put64(&mut b, S_TED, s_text as u64);
    put64(&mut b, S_TED + 8, s_tmplt as u64);
    put64(&mut b, S_TED + 16, s_valid as u64);
    put16(&mut b, S_TED + TE_TXTLEN as usize, 99);
    put16(&mut b, S_TED + TE_TMPLEN as usize, 99);
    // tedinfo[1] — referenced twice.
    let t1 = S_TED + TEDINFO_SIZE;
    put64(&mut b, t1, s_text as u64);
    put64(&mut b, t1 + 8, s_tmplt as u64);
    put64(&mut b, t1 + 16, s_valid as u64);
    put16(&mut b, t1 + TE_TXTLEN as usize, 99);
    put16(&mut b, t1 + TE_TMPLEN as usize, 99);

    // iconblk[0] — unreferenced; mask and data share the raster bytes.
    put64(&mut b, S_IB + IB_PMASK as usize, s_icon as u64);
    put64(&mut b, S_IB + IB_PDATA as usize, s_icon as u64);
    put64(&mut b, S_IB + IB_PTEXT as usize, s_icon as u64);

    // bitblk[0] — unreferenced.
    put64(&mut b, S_BB + BI_PDATA as usize, s_icon as u64);

    b
}

/// EmuTOS `gemrslib.c:798-805` — every declared record is relocated, whether
/// or not an object refers to it.
#[test]
fn unreferenced_substructures_are_relocated_too() {
    let _g = aes();
    let src = synthetic();
    let (_v, p) = blob(&src);
    // SAFETY: `p` is a writable copy of the blob built above, `src.len()` bytes.
    assert!(
        unsafe { tos_api::aes::resource::relocate_and_register(p, src.len()) },
        "the synthetic v5 blob must load"
    );
    let base = p as u64;
    let total = S_TOTAL as u64;

    let (ret, ted0) = gaddr(R_TEDINFO, 0);
    assert_eq!(ret, 1, "rsrc_gaddr(R_TEDINFO, 0) must resolve");
    assert_eq!(ted0, base + S_TED as u64, "tedinfo[0] sits at the array base");
    for (off, name) in [(TE_PTEXT, "te_ptext"), (TE_PTMPLT, "te_ptmplt"), (TE_PVALID, "te_pvalid")] {
        assert_relocated(rd64(ted0 + off), base, total, &format!("unreferenced tedinfo[0].{name}"));
    }

    let (ret, ib) = gaddr(R_ICONBLK, 0);
    assert_eq!(ret, 1, "rsrc_gaddr(R_ICONBLK, 0) must resolve");
    for (off, name) in [(IB_PMASK, "ib_pmask"), (IB_PDATA, "ib_pdata"), (IB_PTEXT, "ib_ptext")] {
        assert_relocated(rd64(ib + off), base, total, &format!("unreferenced iconblk[0].{name}"));
    }

    let (ret, bb) = gaddr(R_BITBLK, 0);
    assert_eq!(ret, 1, "rsrc_gaddr(R_BITBLK, 0) must resolve");
    assert_relocated(rd64(bb + BI_PDATA), base, total, "unreferenced bitblk[0].bi_pdata");
    rsrc_free();
}

/// The length recompute is part of the same array walk (EmuTOS
/// `fix_tedinfo_std`, `gemrslib.c:655-669`), so it reaches a record no object
/// references — `te_txtlen` 99 in the file over a 4-byte string.
#[test]
fn an_unreferenced_tedinfo_gets_its_lengths_recomputed() {
    let _g = aes();
    let src = synthetic();
    let (_v, p) = blob(&src);
    // SAFETY: `p` is a writable copy of the blob built above, `src.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, src.len()) });

    let (ret, ted0) = gaddr(R_TEDINFO, 0);
    assert_eq!(ret, 1);
    let base = p as u64;
    let ptext = rd64(ted0 + TE_PTEXT);
    let ptmplt = rd64(ted0 + TE_PTMPLT);
    // Checked before they are followed: an unrelocated pointer is a small
    // integer, and `cstrlen` on it would take the whole test binary down
    // instead of reporting the defect.
    assert_relocated(ptext, base, S_TOTAL as u64, "unreferenced tedinfo[0].te_ptext");
    assert_relocated(ptmplt, base, S_TOTAL as u64, "unreferenced tedinfo[0].te_ptmplt");
    assert_eq!(
        rd16(ted0 + TE_TXTLEN) as usize,
        cstrlen(ptext) + 1,
        "unreferenced tedinfo[0].te_txtlen was not recomputed; objc_edit \
         would write te_txtlen bytes into a strlen+1-byte buffer"
    );
    assert_eq!(
        rd16(ted0 + TE_TMPLEN) as usize,
        cstrlen(ptmplt) + 1,
        "unreferenced tedinfo[0].te_tmplen was not recomputed"
    );
    rsrc_free();
}

/// A record two objects share is still relocated exactly **once** — the
/// array walk visits each record by index, so double relocation (`base +
/// base + offset`) cannot arise.
#[test]
fn a_shared_tedinfo_is_relocated_exactly_once() {
    let _g = aes();
    let src = synthetic();
    let (_v, p) = blob(&src);
    // SAFETY: `p` is a writable copy of the blob built above, `src.len()` bytes.
    assert!(unsafe { tos_api::aes::resource::relocate_and_register(p, src.len()) });
    let base = p as u64;

    let (ret, ted1) = gaddr(R_TEDINFO, 1);
    assert_eq!(ret, 1, "rsrc_gaddr(R_TEDINFO, 1) must resolve");
    // Both G_BOXTEXT objects point at this record…
    let spec1 = rd64(base + (S_OBJECTS + OBJECT_SIZE + 16) as u64);
    let spec2 = rd64(base + (S_OBJECTS + 2 * OBJECT_SIZE + 16) as u64);
    assert_eq!(spec1, ted1, "object 1's ob_spec is tedinfo[1]");
    assert_eq!(spec2, ted1, "object 2's ob_spec is tedinfo[1]");
    // …and its text pointer is the blob base plus the file offset, once.
    assert_eq!(
        rd64(ted1 + TE_PTEXT),
        base + S_STRINGS as u64,
        "the shared tedinfo[1].te_ptext was relocated more than once"
    );
    rsrc_free();
}
