//! ARCH03 stage 3a: the GEMDOS file surface reads and writes its pointer
//! arguments in the **caller's space** (`TrapArgs::space`) — an EL0 program's
//! through the fault-safe `tos_api::uaccess` routines, an in-kernel caller's
//! (an `svc` from EL1 such as EmuCON's, the AES fiber) as kernel addresses.
//!
//! On the host both spaces are plain host-memory accesses, so what can be
//! asserted here is that the conversion from raw dereferences to the
//! space-aware copies changed no result: every call gives the same return
//! value and the same bytes whether it is issued as `User` or as `Kernel`.
//! The fault side (a bad pointer ends the calling program) needs the MMU and
//! is driven in QEMU (`measurements/measurement-2026-10-05-arch03-3a`, probe
//! UACC3A).

mod fatvol;

use fatvol::*;
use tos_api::uaccess::Space;

const FDATIME: u16 = 0x57;
const DFREE: u16 = 0x36;
const FRENAME: u16 = 0x56;
const FATTRIB: u16 = 0x43;
const DSETPATH: u16 = 0x3B;

fn both<T: PartialEq + std::fmt::Debug>(mut f: impl FnMut(Space) -> T) {
    let k = f(Space::Kernel);
    let u = f(Space::User);
    assert_eq!(k, u, "User and Kernel space must give the same result");
}

#[test]
fn fopen_fread_same_in_both_spaces() {
    let (_g, _host) = fs();
    both(|sp| {
        let path = cpath("C:\\HELLO.TXT");
        let h = call_in(sp, FOPEN, [path.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]);
        assert!(h >= 0, "Fopen ({sp:?}) -> {h}");
        let mut buf = [0xEEu8; 8];
        let n = call_in(sp, FREAD, [h as u64, 8, buf.as_mut_ptr() as u64, 0, 0, 0, 0, 0]);
        assert_eq!(call_in(sp, FCLOSE, [h as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
        (n, buf)
    });
}

#[test]
fn fwrite_then_fread_round_trip_in_user_space() {
    let (_g, _host) = fs();
    let path = cpath("C:\\U3A.TXT");
    let h = call_in(Space::User, FCREATE, [path.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]);
    assert!(h >= 0, "Fcreate -> {h}");
    // Longer than a sector and not a multiple of 8: exercises the partial
    // sector path and the byte tail of the copy routine.
    let data: Vec<u8> = (0..1301u32).map(|i| (i * 7 + 3) as u8).collect();
    let n = call_in(Space::User, FWRITE, [h as u64, data.len() as u64, data.as_ptr() as u64, 0, 0, 0, 0, 0]);
    assert_eq!(n, data.len() as i32);
    assert_eq!(call_in(Space::User, FCLOSE, [h as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);

    let h = call_in(Space::User, FOPEN, [path.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]);
    assert!(h >= 0);
    let mut back = vec![0u8; data.len() + 16];
    let n = call_in(Space::User, FREAD, [h as u64, back.len() as u64, back.as_mut_ptr() as u64, 0, 0, 0, 0, 0]);
    assert_eq!(n, data.len() as i32, "Fread stops at EOF");
    assert_eq!(&back[..data.len()], &data[..]);
    assert!(back[data.len()..].iter().all(|&b| b == 0), "nothing written past the count");
    assert_eq!(call_in(Space::User, FCLOSE, [h as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
    assert_eq!(call_in(Space::User, FDELETE, [path.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
}

#[test]
fn fdatime_get_same_in_both_spaces() {
    let (_g, _host) = fs();
    both(|sp| {
        let path = cpath("C:\\HELLO.TXT");
        let h = call_in(sp, FOPEN, [path.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]);
        assert!(h >= 0);
        let mut stamp = [0u8; 4];
        let rc = call_in(sp, FDATIME, [stamp.as_mut_ptr() as u64, h as u64, 0, 0, 0, 0, 0, 0]);
        let _ = call_in(sp, FCLOSE, [h as u64, 0, 0, 0, 0, 0, 0, 0]);
        (rc, stamp)
    });
}

#[test]
fn fdatime_set_reads_the_callers_stamp() {
    let (_g, _host) = fs();
    let path = cpath("C:\\HELLO.TXT");
    let h = call_in(Space::User, FOPEN, [path.as_ptr() as u64, 2, 0, 0, 0, 0, 0, 0]);
    assert!(h >= 0);
    let mut old = [0u8; 4];
    assert_eq!(call_in(Space::User, FDATIME, [old.as_mut_ptr() as u64, h as u64, 0, 0, 0, 0, 0, 0]), E_OK);
    let new = [0x21u8, 0x43, 0x65, 0x07];
    assert_eq!(call_in(Space::User, FDATIME, [new.as_ptr() as u64, h as u64, 1, 0, 0, 0, 0, 0]), E_OK);
    let mut got = [0u8; 4];
    assert_eq!(call_in(Space::User, FDATIME, [got.as_mut_ptr() as u64, h as u64, 0, 0, 0, 0, 0, 0]), E_OK);
    assert_eq!(got, new);
    assert_eq!(call_in(Space::User, FDATIME, [old.as_ptr() as u64, h as u64, 1, 0, 0, 0, 0, 0]), E_OK);
    let _ = call_in(Space::User, FCLOSE, [h as u64, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn dfree_and_dgetpath_same_in_both_spaces() {
    let (_g, _host) = fs();
    both(|sp| {
        let mut info = [0u8; 16];
        let rc = call_in(sp, DFREE, [info.as_mut_ptr() as u64, 3, 0, 0, 0, 0, 0, 0]);
        let sub = cpath("C:\\SUB");
        assert_eq!(call_in(sp, DSETPATH, [sub.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
        let mut p = [0xAAu8; 32];
        let rc2 = call_in(sp, DGETPATH, [p.as_mut_ptr() as u64, 3, 0, 0, 0, 0, 0, 0]);
        let root = cpath("C:\\");
        assert_eq!(call_in(sp, DSETPATH, [root.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
        (rc, info, rc2, p)
    });
}

#[test]
fn dgetpath_writes_path_and_terminator() {
    let (_g, _host) = fs();
    let sub = cpath("C:\\SUB");
    assert_eq!(call_in(Space::User, DSETPATH, [sub.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
    let mut p = [0xAAu8; 16];
    assert_eq!(call_in(Space::User, DGETPATH, [p.as_mut_ptr() as u64, 3, 0, 0, 0, 0, 0, 0]), E_OK);
    assert_eq!(&p[..5], b"\\SUB\0");
    assert_eq!(p[5], 0xAA, "nothing past the terminator");
    let root = cpath("C:\\");
    assert_eq!(call_in(Space::User, DSETPATH, [root.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
}

#[test]
fn fsfirst_fills_the_same_dta_in_both_spaces() {
    let (_g, _host) = fs();
    both(|sp| {
        let mut dta = [0u8; 44];
        // The process-less caller's DTA cell (`find::KERNEL_DTA`).
        assert_eq!(call_in(sp, FSETDTA, [dta.as_mut_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), E_OK);
        let pat = cpath("C:\\*.*");
        let rc = call_in(sp, FSFIRST, [pat.as_ptr() as u64, FA_ANY, 0, 0, 0, 0, 0, 0]);
        let first = dta;
        let rc2 = call_in(sp, FSNEXT, [0; 8]);
        (rc, first, rc2, dta)
    });
}

#[test]
fn frename_and_fattrib_take_their_paths_in_the_callers_space() {
    let (_g, _host) = fs();
    let a = cpath("C:\\HELLO.TXT");
    let b = cpath("C:\\HELLO2.TXT");
    assert_eq!(call_in(Space::User, FRENAME, [0, a.as_ptr() as u64, b.as_ptr() as u64, 0, 0, 0, 0, 0]), E_OK);
    assert!(call_in(Space::User, FATTRIB, [b.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]) >= 0);
    assert_eq!(call_in(Space::User, FATTRIB, [a.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), EFILNF);
    assert_eq!(call_in(Space::Kernel, FRENAME, [0, b.as_ptr() as u64, a.as_ptr() as u64, 0, 0, 0, 0, 0]), E_OK);
}

#[test]
fn a_path_is_read_up_to_128_bytes_and_a_null_path_is_refused() {
    let (_g, _host) = fs();
    // Null pointer: the call's own error code, as before (`path_arg` → None).
    assert_eq!(call_in(Space::User, FOPEN, [0, 0, 0, 0, 0, 0, 0, 0]), EFILNF);
    assert_eq!(call_in(Space::User, FCREATE, [0, 0, 0, 0, 0, 0, 0, 0]), EPTHNF);
    // "C:\HELLO.TXT" followed by 200 bytes of junk without a NUL inside the
    // first 128: the reader stops at 128 bytes and does not run on.
    let mut long = b"C:\\HELLO.TXT".to_vec();
    long.extend(std::iter::repeat_n(b'X', 200));
    let rc = call_in(Space::User, FOPEN, [long.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]);
    assert!(rc < 0, "a 128-byte name that is not HELLO.TXT does not open it: {rc}");
}
