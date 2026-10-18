//! TASK/CON01 — the BDOS console layer of EmuTOS `bdos/console.c`: the
//! per-device column counter `glbcolumn[]`, the 64-key BDOS typeahead of AUX
//! and CON, `conbrk` (Ctrl-C flush + terminate, Ctrl-S pause until Ctrl-Q,
//! Ctrl-X flush, every other key into the typeahead, bell when full), tab
//! expansion in `tabout` (`Cconout`, `Cconws`, `Fwrite` to CON), the `cgets`
//! echo (`cookdout`, `backsp`, `newline(h, stcol)`) and `getch` preferring the
//! typeahead (`references/emutos/bdos/console.c:27-582`).
//!
//! Spec: `docs/api/gemdos/console/{01,02,06,07,08,09,0a,0b}-*.md` "NewTOS
//! implementation notes" (TASK/CON01, user decision 2026-10-18),
//! `docs/api/gemdos/file/40-fwrite.md`.
//!
//! The host here is a scripted console: keys queued up front, keys that
//! "arrive" once a given number of bytes has been written (a key pressed
//! during output), and keys that arrive only when the BDOS *blocks* on the
//! keyboard (a key pressed while the output is paused) — the blocking read
//! records how much output had been written at that moment.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::bdos::process;
use tos_api::bios::chardev::{install_device, CharDevice};
use tos_api::host::Host;
use tos_api::trap::{TrapArgs, TrapClass};
use tos_types::BasePage;

const CCONIN: u16 = 0x01;
const CCONOUT: u16 = 0x02;
const CRAWIO: u16 = 0x06;
const CRAWCIN: u16 = 0x07;
const CNECIN: u16 = 0x08;
const CCONWS: u16 = 0x09;
const CCONRS: u16 = 0x0A;
const CCONIS: u16 = 0x0B;
const FREAD: u16 = 0x3F;
const FWRITE: u16 = 0x40;
const FFORCE: u16 = 0x46;

const TAB: u8 = 0x09;
const BS: u8 = 0x08;
const CR: u8 = 0x0D;
const LF: u8 = 0x0A;
const CTRL_A: u8 = 0x01;
const CTRL_C: u8 = 0x03;
const CTRL_Q: u8 = 0x11;
const CTRL_R: u8 = 0x12;
const CTRL_S: u8 = 0x13;
const CTRL_U: u8 = 0x15;
const CTRL_X: u8 = 0x18;

// ── scripted console host ─────────────────────────────────────────────────

#[derive(Default)]
struct Script {
    /// The key ring (IKBD key words).
    keys: VecDeque<u16>,
    /// Every byte written to the console.
    out: Vec<u8>,
    /// `(n, keys)`: once `out.len() >= n`, `keys` join the ring.
    at_output: Vec<(usize, Vec<u16>)>,
    /// Keys that arrive one per blocking read on an empty ring.
    on_block: VecDeque<u16>,
    /// `out.len()` at every blocking read that had to wait.
    blocked_at: Vec<usize>,
}

impl Script {
    fn release_due(&mut self) {
        let n = self.out.len();
        let mut i = 0;
        while i < self.at_output.len() {
            if self.at_output[i].0 <= n {
                let (_, k) = self.at_output.remove(i);
                self.keys.extend(k);
            } else {
                i += 1;
            }
        }
    }
}

struct ConHost(Mutex<Script>);

impl ConHost {
    fn s(&self) -> MutexGuard<'_, Script> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn blocking_key(&self) -> u16 {
        let mut s = self.s();
        s.release_due();
        if let Some(k) = s.keys.pop_front() {
            return k;
        }
        let n = s.out.len();
        s.blocked_at.push(n);
        s.on_block.pop_front().expect("blocking console read with nothing left to arrive")
    }
}

impl Host for ConHost {
    fn console_write_byte(&self, byte: u8) {
        let mut s = self.s();
        s.out.push(byte);
        s.release_due();
    }
    fn console_try_read(&self) -> Option<u8> {
        self.console_try_read_key().map(|w| w as u8)
    }
    fn console_try_read_key(&self) -> Option<u16> {
        let mut s = self.s();
        s.release_due();
        s.keys.pop_front()
    }
    fn console_has_input(&self) -> bool {
        let mut s = self.s();
        s.release_due();
        !s.keys.is_empty()
    }
    fn console_read_byte_blocking(&self) -> u8 {
        loop {
            let w = self.blocking_key();
            if w & 0xFF != 0 {
                return w as u8;
            }
        }
    }
    fn console_read_key_blocking(&self) -> u16 {
        self.blocking_key()
    }
    fn console_read_key_restartable(&self) -> Option<u16> {
        Some(self.blocking_key())
    }
}

static HOST: OnceLock<&'static ConHost> = OnceLock::new();
static LOCK: Mutex<()> = Mutex::new(());

fn host() -> &'static ConHost {
    HOST.get_or_init(|| {
        let h: &'static ConHost = Box::leak(Box::new(ConHost(Mutex::new(Script::default()))));
        let slot: &'static &'static dyn Host = Box::leak(Box::new(h as &'static dyn Host));
        tos_api::host::install_host(slot);
        h
    })
}

fn call(func: u16, regs: [u64; 8]) -> i32 {
    let args = TrapArgs { regs, func: func as u64, svc_imm: TrapClass::Gemdos as u16, space: tos_api::uaccess::Space::Kernel };
    tos_api::gemdos::dispatch(&args)
}
fn c0(func: u16) -> i32 {
    call(func, [0; 8])
}
fn c1(func: u16, a: u64) -> i32 {
    call(func, [a, 0, 0, 0, 0, 0, 0, 0])
}

/// Serialize the tests and start each from a known console state, using only
/// the public interface: empty key ring, the BDOS typeahead drained through
/// `Crawcin` while `Cconis` says input is there, column 0 by a CR, no
/// pending terminate, no captured output.
fn setup() -> (MutexGuard<'static, ()>, &'static ConHost) {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let h = host();
    process::set_current(core::ptr::null_mut());
    *h.s() = Script::default();
    while c0(CCONIS) != 0 {
        let _ = c0(CRAWCIN);
    }
    let _ = c1(CCONOUT, CR as u64);
    let _ = process::take_pending_terminate();
    *h.s() = Script::default();
    (g, h)
}

fn keys(h: &ConHost, k: &[u8]) {
    h.s().keys.extend(k.iter().map(|&b| b as u16));
}
fn keys_at(h: &ConHost, n: usize, k: &[u8]) {
    h.s().at_output.push((n, k.iter().map(|&b| b as u16).collect()));
}
fn on_block(h: &ConHost, k: &[u8]) {
    h.s().on_block.extend(k.iter().map(|&b| b as u16));
}
fn out(h: &ConHost) -> Vec<u8> {
    core::mem::take(&mut h.s().out)
}
fn ring(h: &ConHost) -> Vec<u16> {
    h.s().keys.iter().copied().collect()
}
fn blocked_at(h: &ConHost) -> Vec<usize> {
    core::mem::take(&mut h.s().blocked_at)
}

fn cstr(s: &[u8]) -> Vec<u8> {
    let mut v = s.to_vec();
    v.push(0);
    v
}
fn cconws(s: &[u8]) -> i32 {
    let s = cstr(s);
    c1(CCONWS, s.as_ptr() as u64)
}
fn fwrite(h: i32, data: &[u8]) -> i32 {
    call(FWRITE, [h as u16 as u64, data.len() as u64, data.as_ptr() as u64, 0, 0, 0, 0, 0])
}
fn fread(h: i32, buf: &mut [u8]) -> i32 {
    call(FREAD, [h as u16 as u64, buf.len() as u64, buf.as_mut_ptr() as u64, 0, 0, 0, 0, 0])
}
fn cconrs(max: u8) -> (i32, Vec<u8>) {
    let mut rec = vec![0xAAu8; 2 + max as usize + 2];
    rec[0] = max;
    let r = c1(CCONRS, rec.as_mut_ptr() as u64);
    (r, rec)
}
fn spaces(n: usize) -> Vec<u8> {
    vec![b' '; n]
}
fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

// ── tab expansion from the tracked column (`tabout`, `console.c:292-303`) ──

/// A TAB goes to the next multiple of 8 of the tracked column, always at
/// least one space: from 0 → 8, after "ab" → 6, after 8 characters → 8 more
/// (column 16), after 9 characters → 7 (column 16).
#[test]
fn tab_expands_to_column_8_and_16_after_prefixes() {
    let (_g, h) = setup();
    assert_eq!(c1(CCONOUT, TAB as u64), 1);
    assert_eq!(out(h), spaces(8), "TAB at column 0");

    let _ = c1(CCONOUT, CR as u64);
    assert_eq!(cconws(b"ab\tX"), 4, "Cconws counts the string bytes");
    assert_eq!(out(h), cat(&[b"\rab", &spaces(6), b"X"]), "TAB after 2 columns");

    let _ = c1(CCONOUT, CR as u64);
    assert_eq!(cconws(b"12345678\tY"), 10);
    assert_eq!(out(h), cat(&[b"\r12345678", &spaces(8), b"Y"]), "TAB at column 8 → 16");

    let _ = c1(CCONOUT, CR as u64);
    assert_eq!(cconws(b"123456789\tZ\tW"), 13);
    assert_eq!(out(h), cat(&[b"\r123456789", &spaces(7), b"Z", &spaces(7), b"W"]), "column 9 → 16, then 17 → 24");
}

/// CR resets the column, BS steps it back, LF and other control bytes leave
/// it alone (`conout`, `console.c:267-272`).
#[test]
fn column_tracks_cr_bs_and_control_bytes() {
    let (_g, h) = setup();
    assert_eq!(cconws(b"abc\x08\t."), 6);
    // "abc" → 3, BS → 2, TAB → 6 spaces to 8.
    assert_eq!(out(h), cat(&[b"abc\x08", &spaces(6), b"."]));
    assert_eq!(cconws(b"\n\x07\t+"), 4);
    // LF and BEL do not move the column (still 9): TAB → 7 spaces to 16.
    assert_eq!(out(h), cat(&[b"\n\x07", &spaces(7), b"+"]));
    assert_eq!(cconws(b"x\r\t"), 3);
    assert_eq!(out(h), cat(&[b"x\r", &spaces(8)]), "column reset by CR");
}

/// `Fwrite` to CON is `tabout` per byte (`bdosmain.c:631`), sharing the column
/// with `Cconws`/`Cconout` (40-fwrite.md W3-W8).
#[test]
fn fwrite_to_con_expands_tabs_from_the_shared_column() {
    let (_g, h) = setup();
    assert_eq!(fwrite(-1, b"\tX"), 2, "W3");
    assert_eq!(out(h), cat(&[&spaces(8), b"X"]));
    let _ = c1(CCONOUT, CR as u64);
    let _ = out(h);
    assert_eq!(fwrite(-1, b"12345"), 5);
    assert_eq!(fwrite(-1, b"\tX"), 2, "W5");
    assert_eq!(out(h), cat(&[b"12345", &spaces(3), b"X"]));
    let _ = c1(CCONOUT, CR as u64);
    let _ = out(h);
    assert_eq!(fwrite(-1, b"a\rb"), 3, "W8");
    assert_eq!(fwrite(-1, b"\t"), 1);
    assert_eq!(out(h), cat(&[b"a\rb", &spaces(7)]));
    assert_eq!(cconws(b"\t"), 1, "Cconws continues the same column (8)");
    assert_eq!(out(h), spaces(8));
}

/// A test character device for AUX: no input, captures output, accepts.
struct AuxDev(Mutex<Vec<u8>>);
impl CharDevice for AuxDev {
    fn bconstat(&self) -> i32 {
        0
    }
    fn bconin(&self) -> i32 {
        0
    }
    fn bconout(&self, b: u8) -> i32 {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(b);
        1
    }
    fn bcostat(&self) -> i32 {
        -1
    }
}
static AUX: AuxDev = AuxDev(Mutex::new(Vec::new()));
static AUX_REF: &dyn CharDevice = &AUX;

/// Installs [`AUX`] as BIOS device 1 and a process with the default handles
/// for the life of the guard (restored also when the test fails).
struct AuxGuard {
    prev: &'static &'static dyn CharDevice,
    bp: *mut BasePage,
}
impl AuxGuard {
    fn new() -> AuxGuard {
        let bp = Box::into_raw(Box::new([0u8; 256])) as *mut BasePage;
        // SAFETY: fresh box.
        unsafe { (*bp).p_uft = [-1, -1, -2, -3, -1, -1] };
        process::set_current(bp);
        AUX.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
        AuxGuard { prev: install_device(1, &AUX_REF).unwrap(), bp }
    }
}
impl Drop for AuxGuard {
    fn drop(&mut self) {
        install_device(1, self.prev);
        process::set_current(core::ptr::null_mut());
        // SAFETY: nothing names the box any more.
        drop(unsafe { Box::from_raw(self.bp as *mut [u8; 256]) });
    }
}

/// `glbcolumn` is per device (`console.c:54`): stdout forced to AUX tracks
/// AUX's own column, CON's column is untouched by it.
#[test]
fn column_is_per_device() {
    let (_g, h) = setup();
    let _aux = AuxGuard::new();

    assert_eq!(cconws(b"abc"), 3, "CON at column 3");
    assert_eq!(call(FFORCE, [1, (-2i16) as u16 as u64, 0, 0, 0, 0, 0, 0]), 0);
    assert_eq!(cconws(b"\r\t1"), 3, "AUX: column 0 → TAB = 8");
    assert_eq!(call(FFORCE, [1, (-1i16) as u16 as u64, 0, 0, 0, 0, 0, 0]), 0);
    assert_eq!(cconws(b"\t"), 1, "CON still at column 3 → 5 spaces");

    let aux = AUX.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    assert_eq!(aux, cat(&[b"\r", &spaces(8), b"1"]));
    assert_eq!(out(h), cat(&[b"abc", &spaces(5)]));
}

// ── conbrk: Ctrl-S / Ctrl-Q / Ctrl-X / Ctrl-C, typeahead ─────────────────

/// Ctrl-S pressed during `Cconws` stops the output before the next byte; the
/// call waits on the keyboard until Ctrl-Q, then finishes the string
/// (`conbrk`, `console.c:206-244`). Both control keys are consumed.
#[test]
fn ctrl_s_pauses_cconws_until_ctrl_q() {
    let (_g, h) = setup();
    keys_at(h, 3, &[CTRL_S]);
    on_block(h, &[CTRL_Q]);
    assert_eq!(cconws(b"abcdef"), 6);
    assert_eq!(blocked_at(h), vec![3], "paused after \"abc\"");
    assert_eq!(out(h), b"abcdef");
    assert!(ring(h).is_empty());
    assert_eq!(c0(CCONIS), 0, "neither control key was stored");
    assert!(process::take_pending_terminate().is_none());
}

/// A key typed during the pause is stored in the typeahead (the pause loop
/// keeps reading), and comes back on the next `Cconin`.
#[test]
fn key_typed_while_paused_is_kept() {
    let (_g, h) = setup();
    keys(h, &[CTRL_S]);
    on_block(h, &[b'x', CTRL_Q]);
    assert_eq!(cconws(b"ok"), 2);
    assert_eq!(blocked_at(h), vec![0, 0], "paused before the first byte, two keys awaited");
    assert_eq!(out(h), b"ok");
    assert_eq!(c0(CCONIS), -1, "the typeahead holds 'x'");
    assert_eq!(c0(CCONIN) & 0xFF, b'x' as i32);
    assert_eq!(out(h), b"x", "echo");
}

/// Ctrl-X flushes the typeahead and is itself stored (`FALLTHROUGH` into
/// `default`, `console.c:232-240`); keys after it are kept.
#[test]
fn ctrl_x_flushes_the_typeahead() {
    let (_g, h) = setup();
    keys(h, &[b'a', b'b', CTRL_X, b'c']);
    assert_eq!(cconws(b"123456"), 6);
    assert_eq!(out(h), b"123456");
    assert!(ring(h).is_empty(), "conbrk took one key per byte written");
    assert_eq!(c0(CNECIN) & 0xFF, CTRL_X as i32, "'a' and 'b' were flushed, ^X stored");
    assert_eq!(c0(CNECIN) & 0xFF, b'c' as i32);
    assert_eq!(c0(CCONIS), 0);
}

/// Ctrl-C during output flushes the typeahead, requests `Pterm(-32)` and the
/// rest of the string is not written.
#[test]
fn ctrl_c_during_output_terminates_and_flushes() {
    let (_g, h) = setup();
    keys(h, &[b'a', CTRL_C, b'z']);
    let _ = cconws(b"xyz");
    assert_eq!(process::take_pending_terminate(), Some(-32));
    assert_eq!(out(h), b"x", "stopped before the second byte");
    assert_eq!(ring(h), vec![b'z' as u16], "keys behind the Ctrl-C stay in the ring");
    let _ = c0(CRAWCIN);
    assert_eq!(c0(CCONIS), 0, "'a' was flushed from the typeahead");
}

/// Keys typed during output survive it in the typeahead (with their
/// scancode) and come back, in order, on `Cconin`/`Crawio`/`Cconrs`/`Fread`.
#[test]
fn typeahead_keys_survive_output_and_come_back() {
    let (_g, h) = setup();
    keys(h, &[b'h', b'i']);
    h.s().keys.push_back(0x4800); // up arrow: scancode 0x48, ASCII 0
    keys(h, &[b'q', b'r', CR, b's']);
    assert_eq!(cconws(b"0123456789"), 10);
    assert!(ring(h).is_empty(), "all keys moved into the typeahead");
    let _ = out(h);

    assert_eq!(c0(CCONIS), -1);
    assert_eq!(c0(CCONIN) & 0xFF, b'h' as i32);
    assert_eq!(out(h), b"h", "Cconin echoes");
    assert_eq!(c1(CRAWIO, 0xFF) & 0xFF, b'i' as i32);
    assert_eq!(c0(CRAWCIN), 0x0048_0000, "the whole LONG is kept (scancode)");
    let (r, rec) = cconrs(10);
    assert_eq!((r, rec[1]), (0, 2));
    assert_eq!(&rec[2..4], b"qr");
    assert_eq!(out(h), b"qr\r");
    let mut b = [0u8; 1];
    assert_eq!(fread(-1, &mut b), 1);
    assert_eq!(b[0], b's');
    assert_eq!(c0(CCONIS), 0);
}

/// The typeahead holds 64 keys (`KBBUFSZ`, `fs.h:46`); a 65th rings the bell
/// (`Bconout(h, 7)`) and is dropped.
#[test]
fn full_typeahead_rings_the_bell() {
    let (_g, h) = setup();
    let k: Vec<u8> = (0..65u8).map(|i| b'A' + (i % 26)).collect();
    keys(h, &k);
    let s = vec![b'.'; 66];
    assert_eq!(cconws(&s), 66);
    let o = out(h);
    assert_eq!(o.iter().filter(|&&b| b == 0x07).count(), 1, "one bell");
    assert_eq!(o.len(), 67);
    for (i, &want) in k[..64].iter().enumerate() {
        assert_eq!(c0(CRAWCIN) & 0xFF, want as i32, "typeahead slot {i}");
    }
    assert_eq!(c0(CCONIS), 0, "the 65th key was dropped");
}

/// The output paths with no `conbrk` leave the key ring alone: `Crawio`
/// write, and AUX writes (`Fwrite(-2)`).
#[test]
fn raw_output_does_not_take_keys() {
    let (_g, h) = setup();
    keys(h, &[b'k']);
    assert_eq!(c1(CRAWIO, b'w' as u64), 0);
    assert_eq!(fwrite(-2, b"aux"), 3);
    assert_eq!(ring(h), vec![b'k' as u16]);
    assert_eq!(out(h), b"w");
}

// ── cgets echo (`console.c:476-582`) ──────────────────────────────────────

/// Ctrl-U: `#`, CR LF, re-indent to the start column, line restarted.
#[test]
fn cconrs_ctrl_u_reindents_to_the_start_column() {
    let (_g, h) = setup();
    assert_eq!(cconws(b"> "), 2);
    let _ = out(h);
    keys(h, &[b'a', b'b', CTRL_U, b'c', CR]);
    let (r, rec) = cconrs(20);
    assert_eq!((r, rec[1]), (0, 1));
    assert_eq!(rec[2], b'c');
    assert_eq!(out(h), b"ab#\r\n  c\r");
}

/// Ctrl-R: `#`, newline to the start column, the buffer re-echoed.
#[test]
fn cconrs_ctrl_r_redisplays_at_the_start_column() {
    let (_g, h) = setup();
    assert_eq!(cconws(b"$$$"), 3);
    let _ = out(h);
    keys(h, &[b'a', CTRL_A, CTRL_R, CR]);
    let (r, rec) = cconrs(20);
    assert_eq!((r, rec[1]), (0, 2));
    assert_eq!(&rec[2..4], &[b'a', CTRL_A]);
    assert_eq!(out(h), b"a^A#\r\n   a^A\r");
}

/// TAB and control bytes are stored raw and echoed by `cookdout`: TAB
/// expanded, a control byte as `^X`, a scancode-only key as `^@`; BS erases
/// back to the column recomputed from the buffer.
#[test]
fn cconrs_cookdout_echo_and_backspace_over_wide_chars() {
    let (_g, h) = setup();
    keys(h, &[b'a', TAB, b'b', CR]);
    let (_, rec) = cconrs(20);
    assert_eq!(&rec[1..5], &[3, b'a', TAB, b'b']);
    assert_eq!(out(h), cat(&[b"a", &spaces(7), b"b\r"]));

    let _ = c1(CCONOUT, CR as u64);
    let _ = out(h);
    keys(h, &[b'a', TAB, BS, CTRL_A, BS, b'b', CR]);
    let (_, rec) = cconrs(20);
    assert_eq!(&rec[1..4], &[2, b'a', b'b']);
    let erase = |n: usize| [BS, b' ', BS].repeat(n);
    assert_eq!(out(h), cat(&[b"a", &spaces(7), &erase(7), b"^A", &erase(2), b"b\r"]));

    let _ = c1(CCONOUT, CR as u64);
    let _ = out(h);
    h.s().keys.push_back(0x4800);
    keys(h, &[CR]);
    let (_, rec) = cconrs(20);
    assert_eq!(&rec[1..3], &[1, 0]);
    assert_eq!(out(h), b"^@\r");
}

/// Ctrl-X erases the whole line visually (`backsp` until empty) and LF
/// finishes like CR (echo CR only).
#[test]
fn cconrs_ctrl_x_erases_the_line() {
    let (_g, h) = setup();
    keys(h, &[b'a', b'b', CTRL_X, b'c', LF]);
    let (_, rec) = cconrs(20);
    assert_eq!(&rec[1..3], &[1, b'c']);
    let erase = [BS, b' ', BS].repeat(2);
    assert_eq!(out(h), cat(&[b"ab", &erase, b"c\r"]));
}

/// `Fread(-1, n)` (`cgets`) after `Fread` filled the buffer: the key behind
/// the last one stored was taken by the echo's `conbrk` and is returned by the
/// next read (3f-fread.md R6).
#[test]
fn fread_line_full_keeps_the_next_key_in_the_typeahead() {
    let (_g, h) = setup();
    keys(h, b"abcd");
    let mut b = [0u8; 3];
    assert_eq!(fread(-1, &mut b), 3);
    assert_eq!(&b, b"abc");
    assert!(ring(h).is_empty());
    let mut b = [0u8; 1];
    assert_eq!(fread(-1, &mut b), 1);
    assert_eq!(b[0], b'd');
}
