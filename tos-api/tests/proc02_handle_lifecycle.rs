//! TASK/PROC02 — the life of a redirected standard handle as on EmuTOS:
//! `Fforce` takes a use (`bdos/fshand.c:46-77`), a child inherits the
//! caller's redirected standard handles (`init_pd_files`,
//! `bdos/proc.c:387-399`), `Pterm` closes what the dying process redirected or
//! owns (`ixterm`, `bdos/proc.c:110-123`), `Fclose` reverts a standard handle
//! and counts uses down (`xclose`, `bdos/fsopnclo.c:426-496`), and the second
//! `ixclose` of a shared OFD answers `EINTRN` (`bdos/fsopnclo.c:552-566`).
//!
//! The scenario of `measurements/measurement-2026-10-02-port10b` (mintlib
//! `system("prog >file")`, `sdk/libc/mintlib/posix/system.c:303-340`) is
//! replayed call by call. Spec: `docs/api/gemdos/file/{3e-fclose,45-fdup,
//! 46-fforce}.md`, `docs/api/gemdos/_state.md` §2, `process/4b-pexec.md`
//! edge case 7, `process/4c-pterm.md`.
//!
//! Basepages are host buffers; on the host a PD's user address is its
//! kernel address (`process::kalias` is the identity).

mod fatvol;

use fatvol::*;
use tos_api::bdos::fs::file;
use tos_api::bdos::process::{self, loader, standin};
use tos_types::BasePage;

const FDUP: u16 = 0x45;
const FFORCE: u16 = 0x46;
const FSEEK: u16 = 0x42;
const EIHNDL: i32 = -37;
const EINTRN: i32 = -65;

/// EmuTOS `default_handle` (`bdos/console.c:60-68`).
const DEFAULT: [i8; 6] = [-1, -1, -2, -3, -1, -1];

/// A process basepage in host memory with the default standard handles, made
/// the running process while it lives (`run`).
struct Pd(*mut BasePage);
impl Pd {
    fn new() -> Pd {
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        // SAFETY: fresh box.
        unsafe { (*bp).p_uft = DEFAULT };
        Pd(bp)
    }
    fn run(&self) {
        process::set_current(self.0);
    }
    fn uft(&self) -> [i8; 6] {
        // SAFETY: the box is alive.
        unsafe { (*self.0).p_uft }
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

fn fwrite(h: i32, data: &[u8]) -> i32 {
    call(FWRITE, [h as u64, data.len() as u64, data.as_ptr() as u64, 0, 0, 0, 0, 0])
}

fn fclose(h: i32) -> i32 {
    gemdos1(FCLOSE, h as u16 as u64)
}

fn fdup(h: i32) -> i32 {
    gemdos1(FDUP, h as u64)
}

fn fforce(std: i32, h: i32) -> i32 {
    gemdos2(FFORCE, std as u16 as u64, h as u16 as u64)
}

fn fcreate(name: &str) -> i32 {
    let p = cpath(name);
    let h = gemdos2(FCREATE, p.as_ptr() as u64, 0);
    assert!(h >= 6, "Fcreate {name} -> {h}");
    h
}

/// The whole content of `name`, read back through a fresh handle.
fn content(name: &str) -> Vec<u8> {
    let p = cpath(name);
    let h = gemdos2(FOPEN, p.as_ptr() as u64, 0);
    assert!(h >= 6, "Fopen {name} -> {h}");
    let mut buf = [0u8; 64];
    let n = call(FREAD, [h as u64, buf.len() as u64, buf.as_mut_ptr() as u64, 0, 0, 0, 0, 0]);
    assert!(n >= 0, "Fread {name} -> {n}");
    assert_eq!(fclose(h), E_OK);
    buf[..n as usize].to_vec()
}

/// The length the directory entry of `name` gives (`Fseek(0, h, 2)`).
fn length(name: &str) -> i32 {
    let p = cpath(name);
    let h = gemdos2(FOPEN, p.as_ptr() as u64, 0);
    assert!(h >= 6);
    let n = call(FSEEK, [0, h as u64, 2, 0, 0, 0, 0, 0]);
    assert_eq!(fclose(h), E_OK);
    n
}

/// mintlib `system()`'s restore idiom: the `Fforce` keeps a use on the
/// file, and the `Fdup`'d console copy hands its BIOS handle to handle 1, so
/// closing the copy leaves handle 1 alive (PORT10 check 2).
#[test]
fn handle_1_survives_the_close_of_its_saved_copy() {
    let (_g, _host) = fs();
    let parent = Pd::new();
    parent.run();

    let out = fcreate("C:\\P2A.TXT");
    let saved = fdup(1);
    assert!(saved >= 6, "Fdup(1) -> {saved}");
    assert_eq!(fforce(1, out), E_OK);
    assert_eq!(parent.uft()[1] as i32, out, "p_uft[1] = the file");
    assert_eq!(fwrite(1, b"via 1"), 5, "handle 1 writes the file");

    assert_eq!(fforce(1, saved), E_OK);
    assert_eq!(parent.uft()[1], -1, "the copy's BIOS handle propagates (fshand.c:68-69)");
    assert_eq!(fclose(saved), E_OK);
    assert_eq!(fwrite(1, b"console"), 7, "handle 1 still writes after Fclose(saved)");

    // The file's own handle: still the first close of its OFD.
    assert_eq!(fclose(out), E_OK);
    assert_eq!(content("C:\\P2A.TXT"), b"via 1");
    // `Fforce` took a use the restore did not give back (fshand.c:46-77): the
    // entry is still there, its OFD no longer listed -> EINTRN, then free.
    assert_eq!(fclose(out), EINTRN);
    assert_eq!(fclose(out), EIHNDL, "the entry is free now");
}

/// The whole `system("kid >file")` round trip (PORT10 checks 1 and 2): the
/// child inherits handle 1, writes the file through it, its `Pterm` closes it
/// (first close: listed, flushed), the parent restores handle 1 and its own
/// `Fclose(outfd)` is the second close of the OFD (`EINTRN`, ignored by
/// mintlib).
#[test]
fn a_child_writes_the_redirected_file_and_its_pterm_closes_it() {
    let (_g, _host) = fs();
    let parent = Pd::new();
    parent.run();

    let out = fcreate("C:\\P2B.TXT");
    let saved = fdup(1);
    assert_eq!(fforce(1, out), E_OK);

    // Pexec: the loader completes the child's PD while the parent is `run`.
    let child = Pd::new();
    // SAFETY: the child's basepage is a live host buffer.
    unsafe {
        (*child.0).p_uft = [0; 6];
        loader::init_pd_files(child.0);
    }
    assert_eq!(
        child.uft(),
        [-1, out as i8, -2, -3, -1, -1],
        "p_uft[1] > 0 is ixforce'd, the rest copied (proc.c:391-398)"
    );

    child.run();
    assert_eq!(fwrite(1, b"marker"), 6, "the child's handle 1 is the file");
    // Pterm: ixterm closes p_uft[1] (proc.c:116-119).
    // SAFETY: the child is a live PD, the running one.
    unsafe { file::close_process_files(child.0) };
    parent.run();
    drop(child);

    assert_eq!(content("C:\\P2B.TXT"), b"marker", "the file got the child's output");
    assert_eq!(length("C:\\P2B.TXT"), 6, "and the directory entry its length");

    assert_eq!(fforce(1, saved), E_OK);
    assert_eq!(fclose(saved), E_OK);
    assert_eq!(fclose(out), EINTRN, "second ixclose of the OFD (fsopnclo.c:552-566)");
    assert_eq!(fwrite(1, b"back"), 4, "the parent's handle 1 is the console again");
    // Its Pterm: the entry `Fforce` kept is the parent's (f_own) and goes.
    // SAFETY: the parent is a live PD.
    unsafe { file::close_process_files(parent.0) };
    assert_eq!(fclose(out), EIHNDL);
}

/// `Fclose` of a standard handle reverts it to `default_handle` and closes
/// the handle it was forced to (`bdos/fsopnclo.c:450-458`).
#[test]
fn fclose_of_a_standard_handle_reverts_and_closes_the_forced_file() {
    let (_g, _host) = fs();
    let p = Pd::new();
    p.run();
    let f = fcreate("C:\\P2C.TXT");
    assert_eq!(fforce(1, f), E_OK);
    assert_eq!(fforce(2, -1), E_OK);
    assert_eq!(p.uft(), [-1, f as i8, -1, -3, -1, -1]);

    assert_eq!(fclose(1), E_OK, "first close of the OFD");
    assert_eq!(fclose(2), E_OK, "a BIOS mapping: revert only");
    assert_eq!(p.uft(), DEFAULT, "both back to default_handle (console.c:60-68)");
    assert_eq!(fclose(f), EINTRN, "its own close is the second one");
    assert_eq!(fclose(f), EIHNDL, "use count 2 -> 0: free");
}

/// `Fdup` of a redirected standard handle shares the OFD — and its file
/// position — without a use on the source entry; the OFD outlives the
/// source's close while the duplicate points at it (`sftdel`,
/// `bdos/fsopnclo.c:380-410`).
#[test]
fn fdup_shares_the_ofd_and_keeps_it_alive() {
    let (_g, _host) = fs();
    let p = Pd::new();
    p.run();
    let f = fcreate("C:\\P2D.TXT");
    assert_eq!(fforce(1, f), E_OK);
    let d = fdup(1);
    assert!(d >= 6 && d != f);
    assert_eq!(fwrite(d, b"ab"), 2);
    assert_eq!(fwrite(f, b"cd"), 2, "one position for both");

    assert_eq!(fclose(f), E_OK, "the source's entry: use 2 -> 1, first ixclose");
    assert_eq!(fwrite(d, b"ef"), 2, "the duplicate still has the OFD");
    assert_eq!(fclose(d), EINTRN, "second ixclose; the duplicate's entry goes");
    assert_eq!(fwrite(1, b"gh"), 2, "handle 1 still maps the source entry");
    assert_eq!(fclose(1), EINTRN, "reverts handle 1, the source entry goes");
    assert_eq!(p.uft()[1], -1);
    assert_eq!(content("C:\\P2D.TXT"), b"abcdefgh");
}

/// `Fdup` of a standard handle on a BIOS device stores the BIOS handle;
/// `Fforce` from such an entry propagates it without a use, `Fclose` of the
/// entry just counts it down (`bdos/fshand.c:66-69,115-120`,
/// `bdos/fsopnclo.c:460-472`).
#[test]
fn a_bios_mapped_duplicate_is_not_a_file() {
    let (_g, _host) = fs();
    let p = Pd::new();
    p.run();
    let d = fdup(3);
    assert!(d >= 6);
    assert_eq!(fforce(0, d), E_OK);
    assert_eq!(p.uft()[0], -3, "PRN propagated");
    assert_eq!(fclose(d), E_OK);
    assert_eq!(fclose(d), EIHNDL, "use 1 -> 0, freed");
    assert_eq!(fforce(0, d), EIHNDL, "a free entry has no OFD");
    assert_eq!(fforce(0, 1), EIHNDL, "a standard source");
    assert_eq!(fforce(6, -1), EIHNDL, "a non-standard target");
    assert_eq!(fclose(6 + 75), EIHNDL, "NUMHANDLES");
    assert_eq!(fclose(-3), E_OK);
}

/// `Pterm` closes every file the process still owns (`ixterm`,
/// `bdos/proc.c:121-123`): data and length reach the card, the entry is free.
#[test]
fn pterm_closes_the_files_a_process_left_open() {
    let (_g, _host) = fs();
    let p = Pd::new();
    p.run();
    let h = fcreate("C:\\P2E.TXT");
    assert_eq!(fwrite(h, b"left open"), 9);
    // SAFETY: a live PD, the running one.
    unsafe { file::close_process_files(p.0) };
    drop(p);
    assert_eq!(fwrite(h, b"x"), EIHNDL, "closed by the Pterm");
    assert_eq!(length("C:\\P2E.TXT"), 9);
}

/// A program the kernel starts inherits the standard handles of the
/// kernel's `run` stand-in: `default_handle` (TASK/PPARENT stand-ins,
/// `bdos/console.c:107-112`).
#[test]
fn a_kernel_launch_inherits_the_stand_in_handles() {
    let (_g, _host) = fs();
    // SAFETY: test binary setup; boot drive C:.
    let _ = unsafe { standin::init(2) };
    process::set_current(core::ptr::null_mut());
    let child = Pd::new();
    // SAFETY: a live host buffer.
    unsafe {
        (*child.0).p_uft = [0; 6];
        loader::init_pd_files(child.0);
    }
    assert_eq!(child.uft(), DEFAULT);
}
