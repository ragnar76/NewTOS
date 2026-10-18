//! TASK/PROC04 point 1 (= TASK/CON02) — the GEMDOS character functions follow
//! the caller's standard handles `run->p_uft[]` in the two ways EmuTOS has:
//!
//! * **(A) redirected to a file** (`p_uft[std] > 0`): `osif` answers the call
//!   itself with raw byte I/O on that handle (`bdos/bdosmain.c:498-568`,
//!   `stdio_typ` table `:150-197`): one-byte reads returning the byte
//!   sign-extended or `EOF_INDICATOR` `0x0000ff1a`, writes returning `0`,
//!   `Cconws` raw, `Cconrs` byte-wise up to CR (the byte after it eaten),
//!   `Cconis`/`Cauxis` `0` only at EOF, the output-status calls `-1`.
//!   `Crawio` (fn 6, `stdio_typ 0`) is never answered there.
//! * **(B) redirected to another BIOS device** (`p_uft[std] < 0`): the
//!   `bdos/console.c` bodies use `HXFORM(run->p_uft[n])` = `3 + p_uft[n]` as
//!   the BIOS device (`console.c:161-197,281,333,342,377,401,419,434,447,451,
//!   461,535`).
//!
//! Spec: `docs/api/gemdos/_dispatcher.md` §5, `docs/api/gemdos/_state.md` §2
//! (NewTOS deviation 1), `docs/api/gemdos/console/*.md` "Redirected" edge
//! cases.
//!
//! Basepages are host buffers; on the host a PD's user address is its kernel
//! address.

mod fatvol;

use std::collections::VecDeque;
use std::sync::Mutex;

use fatvol::*;
use tos_api::bdos::process;
use tos_api::bios::chardev::{install_device, CharDevice};
use tos_types::BasePage;

const CCONIN: u16 = 0x01;
const CCONOUT: u16 = 0x02;
const CAUXIN: u16 = 0x03;
const CAUXOUT: u16 = 0x04;
const CPRNOUT: u16 = 0x05;
const CRAWIO: u16 = 0x06;
const CRAWCIN: u16 = 0x07;
const CNECIN: u16 = 0x08;
const CCONWS: u16 = 0x09;
const CCONRS: u16 = 0x0A;
const CCONIS: u16 = 0x0B;
const CCONOS: u16 = 0x10;
const CPRNOS: u16 = 0x11;
const CAUXIS: u16 = 0x12;
const CAUXOS: u16 = 0x13;
const FFORCE: u16 = 0x46;
const FSEEK: u16 = 0x42;

/// EmuTOS `EOF_INDICATOR` (`bdos/bdosmain.c:69`).
const EOF_INDICATOR: i32 = 0x0000_ff1a;

/// EmuTOS `default_handle` (`bdos/console.c:60-68`).
const DEFAULT: [i8; 6] = [-1, -1, -2, -3, -1, -1];

// ── process ───────────────────────────────────────────────────────────────

/// A running process basepage in host memory with the default standard
/// handles (`run`).
struct Pd(*mut BasePage);
impl Pd {
    fn new() -> Pd {
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        // SAFETY: fresh box.
        unsafe { (*bp).p_uft = DEFAULT };
        process::set_current(bp);
        Pd(bp)
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

fn setup() -> (std::sync::MutexGuard<'static, ()>, &'static DiskHost, Pd) {
    let (g, host) = fs();
    host.reset_console();
    let _ = process::take_pending_terminate();
    let pd = Pd::new();
    (g, host, pd)
}

// ── calls ─────────────────────────────────────────────────────────────────

fn c0(func: u16) -> i32 {
    call(func, [0; 8])
}

fn c1(func: u16, a: u64) -> i32 {
    call(func, [a, 0, 0, 0, 0, 0, 0, 0])
}

fn fforce(std: i32, h: i32) -> i32 {
    gemdos2(FFORCE, std as u16 as u64, h as u16 as u64)
}

fn fclose(h: i32) -> i32 {
    gemdos1(FCLOSE, h as u16 as u64)
}

fn ftell(h: i32) -> i32 {
    call(FSEEK, [0, h as u16 as u64, 1, 0, 0, 0, 0, 0])
}

fn fcreate(name: &str) -> i32 {
    let p = cpath(name);
    let h = gemdos2(FCREATE, p.as_ptr() as u64, 0);
    assert!(h >= 6, "Fcreate {name} -> {h}");
    h
}

fn fopen(name: &str) -> i32 {
    let p = cpath(name);
    let h = gemdos2(FOPEN, p.as_ptr() as u64, 2);
    assert!(h >= 6, "Fopen {name} -> {h}");
    h
}

fn fdelete(name: &str) {
    assert_eq!(gemdos1(FDELETE, cpath(name).as_ptr() as u64), E_OK, "Fdelete {name}");
}

/// Undo `Fforce(std, fh)` the EmuTOS way: `Fclose(std)` reverts `p_uft[std]`
/// to its default and closes the file (`xclose`, `bdos/fsopnclo.c:450-458`);
/// then the program's own handle goes (its result is the shared-OFD
/// `EINTRN`, PROC02 — not what this file tests).
fn release(std: i32, fh: i32) {
    assert_eq!(fclose(std), E_OK, "Fclose({std})");
    let _ = fclose(fh);
}

/// A file holding `data`, opened for reading on a fresh handle.
fn file_with(name: &str, data: &[u8]) -> i32 {
    let h = fcreate(name);
    let n = call(FWRITE, [h as u64, data.len() as u64, data.as_ptr() as u64, 0, 0, 0, 0, 0]);
    assert_eq!(n, data.len() as i32);
    assert_eq!(fclose(h), E_OK);
    fopen(name)
}

/// The whole content of the file `name`.
fn content(name: &str) -> Vec<u8> {
    let h = fopen(name);
    let mut buf = vec![0u8; 512];
    let n = call(FREAD, [h as u64, buf.len() as u64, buf.as_mut_ptr() as u64, 0, 0, 0, 0, 0]);
    assert!(n >= 0, "Fread {name} -> {n}");
    assert_eq!(fclose(h), E_OK);
    buf.truncate(n as usize);
    buf
}

fn cconws(s: &[u8]) -> i32 {
    let s = cpath(std::str::from_utf8(s).unwrap());
    c1(CCONWS, s.as_ptr() as u64)
}

/// `Cconrs` into a fresh record with `max` in byte 0; returns
/// `(result, record)`.
fn cconrs(max: u8, size: usize) -> (i32, Vec<u8>) {
    let mut rec = vec![0xAAu8; size];
    rec[0] = max;
    let r = c1(CCONRS, rec.as_mut_ptr() as u64);
    (r, rec)
}

// ── a replaceable device entry ─────────────────────────────────────────────

/// A test character device: `Bconin` pops its input (0 when empty), `Bconout`
/// captures and returns 1, `Bconstat` = -1 while input is queued, `Bcostat` =
/// -1 (distinguishable from the absent entry's 0).
struct TestDev {
    input: Mutex<VecDeque<u8>>,
    out: Mutex<Vec<u8>>,
}

impl TestDev {
    const fn new() -> TestDev {
        TestDev { input: Mutex::new(VecDeque::new()), out: Mutex::new(Vec::new()) }
    }
    fn reset(&self, input: &[u8]) {
        *self.input.lock().unwrap() = input.iter().copied().collect();
        self.out.lock().unwrap().clear();
    }
    fn out(&self) -> Vec<u8> {
        core::mem::take(&mut *self.out.lock().unwrap())
    }
    fn pending(&self) -> usize {
        self.input.lock().unwrap().len()
    }
}

impl CharDevice for TestDev {
    fn bconstat(&self) -> i32 {
        if self.input.lock().unwrap().is_empty() { 0 } else { -1 }
    }
    fn bconin(&self) -> i32 {
        self.input.lock().unwrap().pop_front().map_or(0, |b| b as i32)
    }
    fn bconout(&self, byte: u8) -> i32 {
        self.out.lock().unwrap().push(byte);
        1
    }
    fn bcostat(&self) -> i32 {
        -1
    }
}

static DEV: TestDev = TestDev::new();
static DEV_REF: &dyn CharDevice = &DEV;

/// Install [`DEV`] as BIOS device `dev` for the life of the guard.
struct Installed {
    dev: u16,
    prev: &'static &'static dyn CharDevice,
}
impl Installed {
    fn new(dev: u16) -> Installed {
        let prev = install_device(dev, &DEV_REF).expect("device number in range");
        Installed { dev, prev }
    }
}
impl Drop for Installed {
    fn drop(&mut self) {
        install_device(self.dev, self.prev);
    }
}

// ── (A) redirected to a file ──────────────────────────────────────────────

/// `Cconws` / `Cconout` with stdout forced to a file write the raw bytes to
/// the file (tab not expanded, low byte of the WORD) and return `0`
/// (`bdosmain.c:521-533`); nothing reaches the console. `Cconos` is `-1`.
#[test]
fn a_cconws_cconout_write_the_file() {
    let (_g, host, pd) = setup();
    let fh = fcreate("C:\\RA1.TXT");
    assert_eq!(fforce(1, fh), E_OK);
    assert_eq!(pd.uft()[1], fh as i8);

    assert_eq!(cconws(b"hello\t"), 0, "Cconws redirected returns 0");
    assert_eq!(c1(CCONOUT, b'!' as u64), 0, "Cconout redirected returns 0");
    assert_eq!(c1(CCONOUT, 0x0141), 0, "only the low byte of the WORD");
    assert_eq!(cconws(b""), 0, "empty string");
    assert_eq!(c0(CCONOS), -1, "Cconos redirected");
    assert!(host.take_output().is_empty(), "nothing on the console");

    release(1, fh);
    assert_eq!(pd.uft()[1], -1, "Fclose(1) restored the default");
    assert_eq!(cconws(b"con"), 3, "back on the console: the count");
    assert_eq!(host.take_output(), b"con");
    assert_eq!(content("C:\\RA1.TXT"), b"hello\t!A");
    fdelete("C:\\RA1.TXT");
}

/// Same through a program's address space (ARCH03 3a, `Space::User`).
#[test]
fn a_cconws_user_space() {
    let (_g, host, _pd) = setup();
    let fh = fcreate("C:\\RA2.TXT");
    assert_eq!(fforce(1, fh), E_OK);
    let s = cpath("user");
    assert_eq!(call_in(tos_api::uaccess::Space::User, CCONWS, [s.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]), 0);
    release(1, fh);
    assert!(host.take_output().is_empty());
    assert_eq!(content("C:\\RA2.TXT"), b"user");
    fdelete("C:\\RA2.TXT");
}

/// `Cconin` / `Crawcin` / `Cnecin` with stdin forced to a file read one byte
/// each, sign-extended (`char ctmp`), no echo; at EOF `EOF_INDICATOR`
/// (`bdosmain.c:509-519`). `Cconis` is `-1` before EOF, `0` at EOF
/// (`:555-558`).
#[test]
fn a_input_reads_the_file_then_eof() {
    let (_g, host, _pd) = setup();
    let fh = file_with("C:\\RA3.TXT", &[b'a', 0xE4, 0x03, b'\r']);
    assert_eq!(fforce(0, fh), E_OK);
    host.type_keys(b"k"); // a console key must not be taken

    assert_eq!(c0(CCONIS), -1, "Cconis before EOF");
    assert_eq!(c0(CCONIN), b'a' as i32, "Cconin");
    assert_eq!(c0(CRAWCIN), 0xE4u8 as i8 as i32, "Crawcin: sign-extended");
    assert_eq!(c0(CNECIN), 0x03, "Cnecin: Ctrl-C is data here");
    assert!(process::take_pending_terminate().is_none(), "no terminate for a file Ctrl-C");
    assert_eq!(c0(CCONIN), 0x0D, "Cconin: CR");
    assert_eq!(c0(CCONIS), 0, "Cconis at EOF");
    assert_eq!(c0(CCONIN), EOF_INDICATOR, "Cconin at EOF");
    assert_eq!(c0(CRAWCIN), EOF_INDICATOR, "Crawcin at EOF");
    assert_eq!(c0(CNECIN), EOF_INDICATOR, "Cnecin at EOF");

    assert!(host.take_output().is_empty(), "no echo");
    assert_eq!(host.pending_keys(), vec![b'k' as u16], "console untouched");
    release(0, fh);
    fdelete("C:\\RA3.TXT");
}

/// `Cconrs` with stdin forced to a file (`bdosmain.c:535-553`): bytes one by
/// one into the record, stop at CR (stored, not counted) and eat the byte
/// after it; stop at `max` or at EOF; `p[1]` = count; returns `0`.
#[test]
fn a_cconrs_reads_lines_from_the_file() {
    let (_g, host, _pd) = setup();
    let fh = file_with("C:\\RA4.TXT", b"abc\r\nde\rXfghij\r\n");
    assert_eq!(fforce(0, fh), E_OK);

    let (r, rec) = cconrs(10, 16);
    assert_eq!(r, 0);
    assert_eq!(rec[1], 3);
    assert_eq!(&rec[2..6], b"abc\r", "CR stored behind the data");
    assert_eq!(rec[6], 0xAA, "nothing past the CR");

    let (r, rec) = cconrs(10, 16);
    assert_eq!((r, rec[1]), (0, 2));
    assert_eq!(&rec[2..5], b"de\r");
    // The byte after CR ('X') was eaten, not just an LF.
    let (r, rec) = cconrs(3, 16);
    assert_eq!((r, rec[1]), (0, 3), "max 3 stops after 3 bytes");
    assert_eq!(&rec[2..6], &[b'f', b'g', b'h', 0xAA]);

    let (_, rec) = cconrs(10, 16);
    assert_eq!(rec[1], 2);
    assert_eq!(&rec[2..5], b"ij\r");

    let (r, rec) = cconrs(10, 16);
    assert_eq!((r, rec[1]), (0, 0), "at EOF: empty line");
    assert_eq!(rec[2], 0xAA);

    assert!(host.take_output().is_empty(), "no echo");
    release(0, fh);
    fdelete("C:\\RA4.TXT");
}

/// `max` is read **unsigned** (0..255) and stops the loop — the named
/// deviation of 2026-10-18 (`docs/COMPATIBILITY.md` "Sanctioned deviations",
/// TASK/PROC04). EmuTOS reads it as a signed `char` (`max = *pb2++`, `int
/// max`, `bdosmain.c:537-539`): from 128 on only CR or EOF stops it there, past
/// the record. For `max` <= 127 both agree (`a_cconrs_reads_lines_from_the_file`).
#[test]
fn a_cconrs_max_is_unsigned() {
    let (_g, _host, _pd) = setup();
    // 250 bytes without a CR, then a line end.
    let mut data: Vec<u8> = (0..250u32).map(|i| b'a' + (i % 26) as u8).collect();
    data.extend_from_slice(b"\r\n");
    let fh = file_with("C:\\RA5.TXT", &data);
    assert_eq!(fforce(0, fh), E_OK);
    let (r, rec) = cconrs(200, 2 + 256);
    assert_eq!((r, rec[1]), (0, 200), "stops after max = 200 bytes");
    assert_eq!(&rec[2..202], &data[..200]);
    assert!(rec[202..].iter().all(|&b| b == 0xAA), "nothing written past max");
    assert_eq!(ftell(fh), 200, "exactly max bytes read from the file");
    // 0x80 is 128, not "no limit".
    let (r, rec) = cconrs(0x80, 2 + 256);
    assert_eq!((r, rec[1]), (0, 50), "the remaining 50 bytes up to the CR");
    assert_eq!(&rec[2..53], &[&data[200..250], &b"\r"[..]].concat()[..]);
    release(0, fh);
    fdelete("C:\\RA5.TXT");
}

/// stdaux / stdprn forced to a file: `Cauxout`/`Cprnout` write the byte and
/// return `0`, `Cauxin` reads, `Cauxis` is `0` only at EOF, `Cauxos`/`Cprnos`
/// are `-1` (`bdosmain.c:509-564`).
#[test]
fn a_aux_and_prn_to_files() {
    let (_g, host, _pd) = setup();
    let fa = file_with("C:\\RA6.TXT", b"q");
    let fp = fcreate("C:\\RA7.TXT");
    assert_eq!(fforce(2, fa), E_OK);
    assert_eq!(fforce(3, fp), E_OK);

    assert_eq!(c0(CAUXIS), -1, "Cauxis before EOF");
    assert_eq!(c0(CAUXIN), b'q' as i32, "Cauxin");
    assert_eq!(c0(CAUXIS), 0, "Cauxis at EOF");
    assert_eq!(c0(CAUXIN), EOF_INDICATOR, "Cauxin at EOF");
    assert_eq!(c1(CAUXOUT, b'w' as u64), 0, "Cauxout returns 0");
    assert_eq!(c0(CAUXOS), -1, "Cauxos");
    assert_eq!(c1(CPRNOUT, b'p' as u64), 0, "Cprnout returns 0");
    assert_eq!(c0(CPRNOS), -1, "Cprnos (the absent printer would say 0)");

    release(2, fa);
    release(3, fp);
    assert!(host.take_output().is_empty());
    assert_eq!(content("C:\\RA6.TXT"), b"qw");
    assert_eq!(content("C:\\RA7.TXT"), b"p");
    fdelete("C:\\RA6.TXT");
    fdelete("C:\\RA7.TXT");
}

/// `Crawio` (fn 6, `stdio_typ 0`) is not answered by the file block: with
/// stdin/stdout forced to file handle `h` its body uses BIOS device `3 + h`
/// (`console.c:447,451`) — no file I/O, and that device is past the table
/// (`Bconout`/`constat` answer 0, `bios/bios.c:1008-1010`, `console.c:142`).
#[test]
fn a_crawio_is_not_redirected_to_the_file() {
    let (_g, host, _pd) = setup();
    let fi = file_with("C:\\RA8.TXT", b"r");
    let fo = fcreate("C:\\RA9.TXT");
    assert_eq!(fforce(0, fi), E_OK);
    assert_eq!(fforce(1, fo), E_OK);
    host.type_keys(b"k");

    assert_eq!(c1(CRAWIO, b'z' as u64), 0, "Crawio write");
    assert_eq!(c1(CRAWIO, 0xFF), 0, "Crawio read: device 3+h has no input");
    assert_eq!(ftell(fi), 0, "the input file was not read");
    assert_eq!(ftell(fo), 0, "the output file was not written");
    assert!(host.take_output().is_empty(), "not the console either");
    assert_eq!(host.pending_keys(), vec![b'k' as u16]);

    release(0, fi);
    release(1, fo);
    assert_eq!(c1(CRAWIO, 0xFF), b'k' as i32, "back on the console");
    assert!(content("C:\\RA9.TXT").is_empty());
    fdelete("C:\\RA8.TXT");
    fdelete("C:\\RA9.TXT");
}

// ── (B) redirected to another BIOS device ─────────────────────────────────

/// stdout / stdin forced to AUX (`Fforce(n, -2)`): the console bodies run on
/// BIOS device `HXFORM(-2)` = 1 (`console.c:281,401,419,377,447,451,461,535`).
#[test]
fn b_stdio_forced_to_aux() {
    let (_g, host, _pd) = setup();
    DEV.reset(b"");
    let _aux = Installed::new(1);
    assert_eq!(fforce(1, -2), E_OK);
    assert_eq!(fforce(0, -2), E_OK);

    assert_eq!(cconws(b"hi"), 2, "Cconws: the NewTOS count");
    assert_eq!(c1(CCONOUT, b'x' as u64), 1, "Cconout: 1");
    assert_eq!(c1(CRAWIO, b'y' as u64), 0, "Crawio write");
    assert_eq!(DEV.out(), b"hixy");
    assert_eq!(c0(CCONOS), -1, "Cconos = Bcostat(1)");

    assert_eq!(c0(CCONIS), 0, "Cconis = constat(1), no input");
    DEV.reset(b"kmnab\r");
    assert_eq!(c0(CCONIS), -1, "Cconis = constat(1)");
    assert_eq!(c0(CCONIN), b'k' as i32, "Cconin from AUX");
    assert_eq!(DEV.out(), b"k", "echo on the stdin device");
    assert_eq!(c0(CNECIN), b'm' as i32, "Cnecin from AUX");
    assert_eq!(c0(CRAWCIN), b'n' as i32, "Crawcin from AUX");
    let (r, rec) = cconrs(10, 16);
    assert_eq!((r, rec[1]), (0, 2), "Cconrs from AUX");
    assert_eq!(&rec[2..4], b"ab");
    assert_eq!(DEV.out(), b"ab\r", "line echo on AUX");
    assert_eq!(c1(CRAWIO, 0xFF), 0, "Crawio read: no AUX input");
    DEV.reset(b"t");
    assert_eq!(c1(CRAWIO, 0xFF), b't' as i32, "Crawio read from AUX");

    assert!(host.take_output().is_empty(), "nothing on the console");
    host.type_keys(b"c");
    assert_eq!(c0(CCONIS), 0, "the console key is not stdin");
    host.reset_console();
    assert_eq!(fforce(1, -1), E_OK);
    assert_eq!(fforce(0, -1), E_OK);
}

/// stdaux forced to PRN (`Fforce(2, -3)`): the AUX calls run on BIOS device 0
/// (`console.c:179,197,333,434`); `Cauxis` = `constat(0)` = 0 whatever the
/// device says (`console.c:142-143`).
#[test]
fn b_stdaux_forced_to_prn() {
    let (_g, _host, _pd) = setup();
    DEV.reset(b"i");
    let _prn = Installed::new(0);
    assert_eq!(fforce(2, -3), E_OK);
    assert_eq!(c1(CAUXOUT, b'm' as u64), 1, "Cauxout = Bconout(0)");
    assert_eq!(DEV.out(), b"m");
    assert_eq!(c0(CAUXOS), -1, "Cauxos = Bcostat(0)");
    assert_eq!(c0(CAUXIS), 0, "Cauxis = constat(0) = 0");
    assert_eq!(c0(CAUXIN), b'i' as i32, "Cauxin = Bconin(0)");
    assert_eq!(DEV.pending(), 0);
    assert_eq!(fforce(2, -2), E_OK);
}

/// stdprn forced to CON (`Fforce(3, -1)`): `Cprnout` writes the console.
#[test]
fn b_stdprn_forced_to_con() {
    let (_g, host, _pd) = setup();
    assert_eq!(fforce(3, -1), E_OK);
    assert_eq!(c1(CPRNOUT, b'P' as u64), 1, "Cprnout = Bconout(2)");
    assert_eq!(c0(CPRNOS), -1, "Cprnos = Bcostat(2)");
    assert_eq!(host.take_output(), b"P");
    assert_eq!(fforce(3, -3), E_OK);
    assert_eq!(c1(CPRNOUT, b'P' as u64), 0, "absent printer again");
}

/// A BIOS handle below `H_Print` (`Fforce(1, -4)`) gives `HXFORM` = -1: no
/// device (`bios/bios.c:1008-1010`): output goes nowhere.
#[test]
fn b_stdout_forced_below_prn() {
    let (_g, host, _pd) = setup();
    assert_eq!(fforce(1, -4), E_OK);
    assert_eq!(c1(CCONOUT, b'a' as u64), 1);
    assert_eq!(c0(CCONOS), 0, "Bcostat(-1) = 0");
    assert!(host.take_output().is_empty());
    assert_eq!(fforce(1, -1), E_OK);
}

// ── defaults ──────────────────────────────────────────────────────────────

/// The default handles keep the hard-wired devices: CON for stdin/stdout,
/// AUX for stdaux, PRN for stdprn — for a process and for kernel code with no
/// process (`run` = the stand-in, whose `p_uft` is `default_handle`).
#[test]
fn defaults_unchanged_with_and_without_process() {
    let (_g, host, pd) = setup();
    DEV.reset(b"");
    let _aux = Installed::new(1);
    for with_process in [true, false] {
        if !with_process {
            process::set_current(core::ptr::null_mut());
        }
        assert_eq!(cconws(b"ab"), 2);
        assert_eq!(c1(CCONOUT, b'c' as u64), 1);
        assert_eq!(host.take_output(), b"abc");
        assert_eq!(c1(CAUXOUT, b'x' as u64), 1);
        assert_eq!(DEV.out(), b"x", "stdaux = AUX");
        assert_eq!(c1(CPRNOUT, b'x' as u64), 0, "stdprn = absent PRN");
        host.type_keys(b"q");
        assert_eq!(c0(CCONIS), -1);
        assert_eq!(c0(CCONIN), b'q' as i32);
        assert_eq!(host.take_output(), b"q", "echo");
    }
    process::set_current(pd.0);
}
