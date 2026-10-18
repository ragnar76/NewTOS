//! TASK/PROC04 point 4 — the open-file table has EmuTOS's size: `OPNFILES`
//! 75 entries, handles `NUMSTD..NUMHANDLES` = 6..80 (`bdos/fs.h:43-45`,
//! `sft[OPNFILES]` `bdos/fsglob.c:34`), system-wide, with no `CONF_WITH_*`
//! gate. A full table answers `ENHNDL` from every allocator of an entry:
//! `opnfil` for `Fopen`/`Fcreate` (`bdos/fsopnclo.c:341-346`) and `xdup` for
//! `Fdup` (`bdos/fshand.c:100-105`). `Fcreate`'s directory entry is written
//! before `opnfil` runs (`bdos/fsopnclo.c:186-201`), so the file exists even
//! when no handle is left. Spec: `docs/api/gemdos/_state.md` §2,
//! `docs/api/gemdos/file/3e-fclose.md`.

mod fatvol;

use fatvol::*;
use tos_api::bdos::process;
use tos_types::BasePage;

const FDUP: u16 = 0x45;
const FSEEK: u16 = 0x42;
const EIHNDL: i32 = -37;
const ENHNDL: i32 = -35;

/// EmuTOS `NUMSTD` / `OPNFILES` / `NUMHANDLES`.
const NUMSTD: i32 = 6;
const OPNFILES: i32 = 75;
const NUMHANDLES: i32 = NUMSTD + OPNFILES;

/// EmuTOS `default_handle` (`bdos/console.c:60-68`).
const DEFAULT: [i8; 6] = [-1, -1, -2, -3, -1, -1];

/// A running process basepage in host memory (`run`).
struct Pd(*mut BasePage);
impl Pd {
    fn new() -> Pd {
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        // SAFETY: fresh box.
        unsafe { (*bp).p_uft = DEFAULT };
        process::set_current(bp);
        Pd(bp)
    }
}
impl Drop for Pd {
    fn drop(&mut self) {
        if process::current() == self.0 {
            process::set_current(core::ptr::null_mut());
        }
        // SAFETY: nothing names the box any more.
        drop(unsafe { Box::from_raw(self.0 as *mut [u8; 256]) });
    }
}

fn fopen(name: &str) -> i32 {
    let p = cpath(name);
    gemdos2(FOPEN, p.as_ptr() as u64, 0)
}

fn fcreate(name: &str) -> i32 {
    let p = cpath(name);
    gemdos2(FCREATE, p.as_ptr() as u64, 0)
}

fn fclose(h: i32) -> i32 {
    gemdos1(FCLOSE, h as u16 as u64)
}

fn fdup(h: i32) -> i32 {
    gemdos1(FDUP, h as u64)
}

fn fseek(off: i32, h: i32, mode: u16) -> i32 {
    call(FSEEK, [off as u32 as u64, h as u16 as u64, mode as u64, 0, 0, 0, 0, 0])
}

/// Fill the whole table: one file opened `OPNFILES` times (TOS lets a file be
/// opened more than once). Returns the handles in claim order.
fn fill(name: &str) -> Vec<i32> {
    let h = fcreate(name);
    assert!(h >= NUMSTD, "Fcreate {name} -> {h}");
    assert_eq!(fclose(h), E_OK);
    (0..OPNFILES)
        .map(|i| {
            let h = fopen(name);
            assert_eq!(h, NUMSTD + i, "Fopen #{} -> {h}", i + 1);
            h
        })
        .collect()
}

fn close_all(hs: &[i32]) {
    for &h in hs {
        assert_eq!(fclose(h), E_OK, "Fclose {h}");
    }
}

/// 75 opens succeed on handles 6..80; the 76th `Fopen`, a `Fcreate` and a
/// `Fdup` answer `ENHNDL`; a freed entry is reused first.
#[test]
fn seventy_five_handles_then_enhndl() {
    let (_g, _host) = fs();
    let _p = Pd::new();

    let hs = fill("C:\\P4A.TXT");
    assert_eq!(*hs.last().unwrap(), NUMHANDLES - 1, "last handle is 80");
    assert_eq!(fseek(0, NUMHANDLES - 1, 2), 0, "handle 80 is a live file");

    assert_eq!(fopen("C:\\P4A.TXT"), ENHNDL, "76th Fopen (opnfil)");
    assert_eq!(fcreate("C:\\P4B.TXT"), ENHNDL, "Fcreate on a full table (opnfil)");
    assert_eq!(fdup(1), ENHNDL, "Fdup on a full table (xdup)");

    // `ixcreat` wrote the directory entry before `opnfil` failed.
    assert_eq!(fclose(hs[10]), E_OK);
    let b = fopen("C:\\P4B.TXT");
    assert_eq!(b, hs[10], "the created file exists; the freed entry is reused");
    assert_eq!(fopen("C:\\P4A.TXT"), ENHNDL, "full again");
    assert_eq!(fclose(b), E_OK);

    let d = fdup(1);
    assert_eq!(d, hs[10], "Fdup takes the first free entry");
    assert_eq!(fclose(d), E_OK);

    close_all(&hs[..10]);
    close_all(&hs[11..]);
    assert_eq!(gemdos1(FDELETE, cpath("C:\\P4A.TXT").as_ptr() as u64), E_OK);
    assert_eq!(gemdos1(FDELETE, cpath("C:\\P4B.TXT").as_ptr() as u64), E_OK);
}

/// The `>= NUMHANDLES` contract rows: handle 80 is in range (a free entry is
/// `EIHNDL` only because it is free), 81 is past the table.
#[test]
fn numhandles_is_81() {
    let (_g, _host) = fs();
    let _p = Pd::new();
    let hs = fill("C:\\P4C.TXT");
    assert_eq!(fclose(NUMHANDLES - 1), E_OK, "handle 80 closes");
    assert_eq!(fclose(NUMHANDLES - 1), EIHNDL, "now free");
    assert_eq!(fclose(NUMHANDLES), EIHNDL, "81 is past NUMHANDLES");
    assert_eq!(fseek(0, NUMHANDLES, 0), EIHNDL, "81 is past NUMHANDLES");
    close_all(&hs[..hs.len() - 1]);
    assert_eq!(gemdos1(FDELETE, cpath("C:\\P4C.TXT").as_ptr() as u64), E_OK);
}
