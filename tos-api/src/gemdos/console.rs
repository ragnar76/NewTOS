//! GEMDOS console group — output-side leaf functions (PHASE-02 D3).
//!
//! Specs: `docs/api/gemdos/console/*.md`.
//!
//! Every character-device access goes through the BIOS device table
//! ([`crate::bios::chardev`], FS11): the `Ccon*`/`Caux*`/`Cprn*` calls are the
//! BIOS calls of CON (2) / AUX (1) / PRN (0), and `Fread`/`Fwrite` on a device
//! handle use [`device_read`] / [`device_write`] — EmuTOS' `conin`/`cgets`/
//! `tabout` over `Bconin`/`Bconout` of that device (`bdos/bdosmain.c:604-640`).
//!
//! The calls follow the caller's standard handles `run->p_uft[]` (TASK/PROC04
//! point 1, `docs/api/gemdos/_dispatcher.md` §5) in EmuTOS's two ways: a
//! standard handle forced to a **file** is answered before dispatch by
//! [`redirect_to_file`] (`osif`, `bdos/bdosmain.c:498-568`); a standard
//! handle that is a **BIOS handle** names the device the bodies below use,
//! `HXFORM(run->p_uft[n])` ([`std_dev`], `bdos/console.c`). With the default
//! handles that is CON (2) for stdin/stdout, AUX (1) for stdaux, PRN (0) for
//! stdprn.
//!
//! All functions take a borrow of [`TrapArgs`] and return a `Long`
//! (`i32` in `x0`). The semantics mirror EmuTOS exactly so binary
//! programs see TOS-bit-compatible behaviour; per-function citations
//! point to the EmuTOS source location used to derive the code.

use crate::bios::chardev::{self, pack_console_key, CharDevice, DEV_AUX, DEV_CON};
use crate::host::host;
use crate::trap::TrapArgs;
use tos_types::error::EUNDEV;
use tos_types::Long;

// Helper: fetch the host or fall through with `EUNDEV`. In normal
// operation the host is always installed; the early-return path
// exists so a misconfigured boot fails predictably instead of
// dereferencing a null function pointer.
macro_rules! host_or_eundev {
    () => {
        match host() {
            Some(h) => h,
            None => return EUNDEV,
        }
    };
}

// ---------------------------------------------------------------------------
// Standard handles: BIOS device (`HXFORM`) and the file redirection in `osif`.
// ---------------------------------------------------------------------------

/// Standard input (`p_uft[0]`).
const STDIN: u16 = 0;
/// Standard output (`p_uft[1]`).
const STDOUT: u16 = 1;
/// Standard aux (`p_uft[2]`).
const STDAUX: u16 = 2;
/// Standard printer (`p_uft[3]`).
const STDPRN: u16 = 3;

/// The BIOS device of standard handle `std`: `HXFORM(run->p_uft[std])` =
/// `3 + p_uft[std]` (`bdos/console.h:36`), as the `bdos/console.c` bodies
/// compute it. A handle below `H_Print` gives a negative BIOS handle and a
/// file handle `h` gives `3 + h`; neither names a table entry, so the device
/// calls answer `0` there (EmuTOS `bconout` etc.: `handle` outside 0..7 → 0,
/// `bios/bios.c:930-932,973-976,1008-1010,1160-1162`).
fn std_dev(std: u16) -> u16 {
    hxform(crate::bdos::fs::file::std_handle(std))
}

/// EmuTOS `constat(h)` (`bdos/console.c:136-146`): only AUX (1) and CON (2)
/// have an input status (NewTOS has no BDOS typeahead buffer to look at
/// first); every other device answers `0`.
fn constat(dev: u16) -> Long {
    if dev == DEV_AUX || dev == DEV_CON {
        chardev::bconstat_handle(dev)
    } else {
        0
    }
}

/// EmuTOS `EOF_INDICATOR` (`bdos/bdosmain.c:69`): what a redirected one-byte
/// read returns at end of file or on a read error ("same way as MiNT").
pub const EOF_INDICATOR: Long = 0x0000_ff1a;

/// `funcs[fn].stdio_typ & 0x7f` (`bdos/bdosmain.c:150-197`) for the functions
/// the file block of `osif` covers — `typ && fn && (fn < 12 || 16 <= fn <=
/// 19)` (`:498`). `Crawio` (6) has `stdio_typ 0` and is not among them.
fn redirected_std(fn_id: usize) -> Option<u16> {
    match fn_id {
        0x01 | 0x07 | 0x08 | 0x0A | 0x0B => Some(STDIN),
        0x02 | 0x09 | 0x10 => Some(STDOUT),
        0x03 | 0x04 | 0x12 | 0x13 => Some(STDAUX),
        0x05 | 0x11 => Some(STDPRN),
        _ => None,
    }
}

/// The file block of EmuTOS `osif` (`bdos/bdosmain.c:498-568`,
/// `docs/api/gemdos/_dispatcher.md` §5), run by the dispatcher before the
/// function table: when the call's standard handle is forced to a file
/// (`run->p_uft[std] > 0`) the call is answered here with raw byte I/O on that
/// handle — no echo, no tab expansion, no break check, no line editing — and
/// `Some(result)` is returned; otherwise `None` and the call dispatches
/// normally.
///
/// | fn | answer |
/// |----|--------|
/// | Cconin, Cauxin, Crawcin, Cnecin | one byte read, **sign-extended** (`char ctmp`); [`EOF_INDICATOR`] when it is not read |
/// | Cconout, Cauxout, Cprnout | the low byte of the WORD written; `0` |
/// | Cconws | `strlen(s)` bytes written raw; `0` |
/// | Cconrs | bytes read one by one into `p[2..]` until `max`, EOF or CR (stored, not counted; the byte after it read and dropped); `p[1]` = count; `0` |
/// | Cconis, Cauxis | `0` at EOF (or no file), else `-1` |
/// | Cconos, Cprnos, Cauxos | `-1` |
///
/// The xread/xwrite results are dropped as in EmuTOS, except a device error
/// or a fault: EmuTOS unwinds the whole call there (`longjmp_rwabs`, a bus
/// error), NewTOS returns that code ([`crate::uaccess::FAULTED`] lets the
/// syscall tail end the program). Kernel scratch bytes are accessed as kernel
/// memory, the caller's string and record through its space
/// ([`TrapArgs::space`], ARCH03 3a), so memory protection applies as for
/// `Fread`/`Fwrite`.
pub fn redirect_to_file(fn_id: usize, args: &TrapArgs) -> Option<Long> {
    use crate::bdos::fs::file::{eof, std_handle, xread, xwrite};
    use crate::uaccess::Space;

    let std = redirected_std(fn_id)?;
    let h = std_handle(std);
    if h <= 0 {
        return None; // a BIOS handle: the function body uses `HXFORM`
    }
    // `Err` of xread/xwrite ends the call with its code.
    macro_rules! io {
        ($e:expr) => {
            match $e {
                Ok(v) => v,
                Err(code) => return Some(code),
            }
        };
    }
    let rc = match fn_id {
        // Cconin, Cauxin, Crawcin, Cnecin (`:509-519`).
        0x01 | 0x03 | 0x07 | 0x08 => {
            let mut ctmp = 0u8;
            let p = core::ptr::addr_of_mut!(ctmp);
            if io!(xread(h, 1, Space::Kernel, p as u64)) != 1 {
                EOF_INDICATOR
            } else {
                // SAFETY: `p` is this frame's byte, written by the read above
                // through that address.
                let b = unsafe { p.read_volatile() };
                b as i8 as Long // `char ctmp` → sign-extended
            }
        }
        // Cconout, Cauxout, Cprnout (`:521-528`): the char in the int.
        0x02 | 0x04 | 0x05 => {
            let ch = args.arg_u16(0) as u8;
            let _ = io!(xwrite(h, 1, Space::Kernel, core::ptr::addr_of!(ch) as u64));
            0 // dummy
        }
        // Cconws (`:530-533`).
        0x09 => {
            let s = args.arg_u64(0);
            if s != 0 {
                // `strlen(pb2)` in the caller's space; an unreadable byte ends
                // the call (EmuTOS: bus error).
                let mut len: u32 = 0;
                loop {
                    match args.space.get_u8(s + u64::from(len)) {
                        Ok(0) => break,
                        Ok(_) => len += 1,
                        Err(_) => return Some(crate::uaccess::FAULTED),
                    }
                }
                let _ = io!(xwrite(h, len as i32, args.space, s));
            }
            0 // dummy
        }
        // Cconrs (`:535-553`).
        0x0A => {
            let pb2 = args.arg_u64(0);
            if pb2 == 0 {
                return Some(0);
            }
            // `max = *pb2++` — a plain (signed) `char` into an `int`, and the
            // loop runs `while (max--)`: a value with bit 7 set counts down
            // through the whole `int` range, so only CR or EOF stops it.
            let Ok(m) = args.space.get_u8(pb2) else {
                return Some(crate::uaccess::FAULTED);
            };
            let mut max = m as i8 as i32 as u32;
            let mut i: u32 = 0;
            while max != 0 {
                max -= 1;
                // `p = pb2 + 1 + i` = record byte `2 + i`.
                let p = pb2 + 2 + u64::from(i);
                if io!(xread(h, 1, args.space, p)) != 1 {
                    break;
                }
                let Ok(c) = args.space.get_u8(p) else {
                    return Some(crate::uaccess::FAULTED);
                };
                if c == 0x0D {
                    // Eat the LF — whatever byte follows the CR.
                    let mut ctmp = 0u8;
                    let _ = io!(xread(h, 1, Space::Kernel, core::ptr::addr_of_mut!(ctmp) as u64));
                    break;
                }
                i += 1;
            }
            if args.space.put_u8(pb2 + 1, i as u8).is_err() {
                return Some(crate::uaccess::FAULTED);
            }
            0
        }
        // Cconis, Cauxis (`:555-558`): `eof` is non-zero at EOF and for a
        // handle with no file (EIHNDL).
        0x0B | 0x12 if eof(h) != 0 => 0,
        // Cconis/Cauxis before EOF, Cconos, Cprnos, Cauxos (`:561-564`).
        _ => -1,
    };
    Some(rc)
}

// ---------------------------------------------------------------------------
// Output-side break check — NewTOS' Ctrl-C-only subset of EmuTOS' `conbrk`
// (bdos/console.c:206-244), run before each output byte by Cconout/Cconws.
// ---------------------------------------------------------------------------

/// Console output break check, run before every output byte by
/// [`cconout`] / [`cconws`]. It **non-destructively** peeks the front of
/// the console input queue; if a Ctrl-C (0x03) is pending it consumes that
/// one key and records the deferred `terminate()` (`Pterm(-32)`, the same
/// path as the `Cconrs` Ctrl-C arm), returning `true` so the caller aborts
/// the output. Any other pending key — **including Ctrl-S (0x13) and
/// Ctrl-Q (0x11)** — is left in the queue untouched, so normal type-ahead
/// survives for the next `Cconin`/`Cconrs` (no read-path regress).
///
/// **Divergence from EmuTOS (documented, approved).** EmuTOS' `conbrk`
/// consumes each pending key into a BDOS *typeahead* buffer while scanning
/// for control chars, which also gives it Ctrl-S/Ctrl-Q flow control.
/// NewTOS has a single lock-free SPSC input ring and no typeahead buffer,
/// so a faithful scan-and-keep is not possible without re-architecting the
/// shared console-input path (deferred). NewTOS therefore implements the
/// **Ctrl-C-only** subset: it aborts a running output but does **not** do
/// Ctrl-S/Ctrl-Q flow control (those bytes stay in the queue and are read
/// as data later). A Ctrl-C queued strictly behind unread normal keys is
/// deferred until those keys are read (the peek only sees the front). See
/// `docs/api/gemdos/console/{02-cconout,09-cconws}.md`.
pub(crate) fn output_break(h: &dyn crate::host::Host) -> bool {
    match h.console_peek_key() {
        Some(word) if (word & 0xFF) as u8 == CTRL_C => {
            // Consume exactly the peeked front key, then defer terminate.
            let _ = h.console_try_read_key();
            crate::bdos::process::request_terminate(CTRLC_RC);
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 0x02 Cconout — write a single byte to the console.
// EmuTOS: bdos/console.c:78  `xconout(WORD ch)`.
// Spec:   docs/api/gemdos/console/02-cconout.md
// ---------------------------------------------------------------------------

/// `Cconout(ch)` — write `ch` (low 8 bits) to the console output.
/// Always returns `1` (per `docs/api/gemdos/console/02-cconout.md`
/// and discrepancy `D-001`: EmuTOS unconditionally returns `1`,
/// even though Compendium and Profibuch document the value as
/// "undefined" or "the function code"). NewTOS follows EmuTOS.
///
/// The device is stdout's, `HXFORM(run->p_uft[1])` (`bdos/console.c:281`,
/// [`std_dev`]); CON by default. EmuTOS' `xconout` runs `conbrk` before
/// writing; NewTOS mirrors that on CON with the Ctrl-C-only
/// [`output_break`] — a pending Ctrl-C aborts the write and defers
/// `Pterm(-32)`. On another device there is no break check: every non-CON
/// table entry has no input (`Bconstat` 0), where EmuTOS' `conbrk` does
/// nothing either (the `Fwrite` device path, [`device_write`], does the
/// same). The byte goes out through that device's table entry (`Bconout`,
/// return ignored as in `conout`).
pub fn cconout(args: &TrapArgs) -> Long {
    let ch = args.arg_u16(0) as u8;
    let h = host_or_eundev!();
    let dev = std_dev(STDOUT);
    if dev == DEV_CON && output_break(h) {
        return 1; // Ctrl-C: aborted; SVC handler redirects to Pterm.
    }
    chardev::bconout_handle(dev, ch);
    1
}

// ---------------------------------------------------------------------------
// AUX BIOS handle. EmuTOS' `xauxin`/`xauxout` resolve stdaux from
// `run->p_uft[2]` (default `H_Aux = -2`) and call `Bconin(HXFORM(p_uft[2]))` /
// `Bconout(HXFORM(p_uft[2]), ch)` where `HXFORM(h) = 3 + h`, i.e. BIOS handle 1
// by default (`docs/api/gemdos/console/{03-cauxin,04-cauxout}.md`) — `DEV_AUX`
// of the device table.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 0x03 Cauxin — read a single byte from AUX.
// EmuTOS: bdos/console.c:432 `xauxin()` = `Bconin(1)` — a pure pass-through
// (docs/api/gemdos/console/03-cauxin.md).
// ---------------------------------------------------------------------------

/// `Cauxin()` — pass-through to `Bconin(HXFORM(p_uft[2]))`, exactly as
/// EmuTOS' `xauxin` (`bdos/console.c:434`): by default `Bconin(1)`, the AUX
/// entry of the device table. That entry is "port present, no
/// cable" (`docs/api/gemdos/console/03-cauxin.md`, FS11 note; `3f-fread.md`
/// item 1a), so the pass-through returns `0` at once instead of waiting for
/// a byte that never arrives; a UART driver (`TASK/IO01.md`) replaces the
/// entry.
pub fn cauxin(_: &TrapArgs) -> Long {
    chardev::bconin_handle(std_dev(STDAUX))
}

// ---------------------------------------------------------------------------
// 0x04 Cauxout — write a single byte to AUX.
// EmuTOS: bdos/console.c:331 `xauxout(ch)` = `Bconout(1, ch)` — a pure
// pass-through (docs/api/gemdos/console/04-cauxout.md).
// ---------------------------------------------------------------------------

/// `Cauxout(ch)` — pass-through to `Bconout(HXFORM(p_uft[2]), ch)`, exactly
/// as EmuTOS' `xauxout` (`bdos/console.c:333`): by default the AUX entry of
/// the device table. The no-cable entry
/// discards the byte and returns `1`
/// (`docs/api/gemdos/console/04-cauxout.md`, option A).
pub fn cauxout(args: &TrapArgs) -> Long {
    let ch = args.arg_u16(0) as u8;
    chardev::bconout_handle(std_dev(STDAUX), ch)
}

// ---------------------------------------------------------------------------
// 0x05 Cprnout — write a single byte to the printer.
// EmuTOS: bdos/console.c:340 `xprtout(WORD ch)` = `Bconout(0, ch)`.
// Spec: docs/api/gemdos/console/05-cprnout.md (`0` = printer busy/absent, a
// non-negative ready/busy signal, *not* a TOS error code).
// ---------------------------------------------------------------------------

/// `Cprnout(ch)` — pass-through to `Bconout(HXFORM(p_uft[3]), ch)`
/// (`bdos/console.c:342`): by default `Bconout(0, ch)`, the PRN entry of the
/// device table. With the absent-device entry (no printer port) the byte is
/// never accepted and the call returns `0` at once — the value EmuTOS'
/// `bconout0` returns without `CONF_WITH_PRINTER_PORT`
/// (`bios/parport.c:144-172`; FS11 note in the spec).
pub fn cprnout(args: &TrapArgs) -> Long {
    let ch = args.arg_u16(0) as u8;
    chardev::bconout_handle(std_dev(STDPRN), ch)
}

// ---------------------------------------------------------------------------
// 0x06 Crawio — raw console I/O.
// EmuTOS: bdos/console.c:170 `xrawio(WORD w)`.
// If `w == 0xFF`, attempt to read a character (non-blocking, raw).
// Otherwise write `w` to the console. Returns the read character
// (low byte) on success, 0 if no input available, or for writes
// returns the byte written.
// ---------------------------------------------------------------------------

/// `Crawio(w)` — raw console I/O without echo or Ctrl-C handling.
///
/// Per `docs/api/gemdos/console/06-crawio.md`: the read path
/// (`w == 0x00FF`) is **non-blocking** and returns the packed console
/// LONG (scancode in bits 16-23, ASCII in bits 0-7) or `0` if no input;
/// the write path returns **`0` always** (not the written byte —
/// EmuTOS `xrawio` falls through to `Bconout(1, w)` and discards
/// the BIOS return).
///
/// The read side uses stdin's device, the write side stdout's
/// (`HXFORM(run->p_uft[0])` / `[1]`, `bdos/console.c:447,451`, [`std_dev`]).
/// `Crawio` is never answered by the file redirection ([`redirect_to_file`],
/// `stdio_typ 0`): a standard handle forced to file handle `h` gives device
/// `3 + h`, which no table entry serves — the read answers `0` (`constat`),
/// the write goes nowhere.
pub fn crawio(args: &TrapArgs) -> Long {
    let w = args.arg_u16(0);
    let h = host_or_eundev!();
    if w == 0x00FF {
        let dev = std_dev(STDIN);
        if dev != DEV_CON {
            // `constat(i) ? getch(i) : 0` on that device.
            return if constat(dev) != 0 { chardev::bconin_handle(dev) } else { 0 };
        }
        // Read side: non-blocking; return 0 if no input. Uses the whole
        // key word so a pending non-ASCII key (arrow / function) is not
        // silently dropped.
        match h.console_try_read_key() {
            Some(word) => pack_console_key(word),
            None => 0,
        }
    } else {
        let dev = std_dev(STDOUT);
        if dev == DEV_CON {
            h.console_write_byte(w as u8);
        } else {
            chardev::bconout_handle(dev, w as u8);
        }
        0
    }
}

// ---------------------------------------------------------------------------
// Blocking console input (PHASE-02 D3, unblocked by PHASE-06).
//
// For the CON handle the blocking reads return the packed console LONG
// (scancode in bits 16-23, ASCII in bits 0-7 — see `pack_console_key` and
// `docs/api/gemdos/console/01-cconin.md`), built from the IKBD key word
// delivered by `Host::console_read_key_blocking`. Non-ASCII keys
// (arrows / function keys) arrive with ASCII 0 and a non-zero scancode.
// Ctrl-C (ASCII 0x03) maps to `terminate()` = `Pterm(-32)`; a leaf GEMDOS
// function cannot redirect the trap frame, so it records the request via
// [`crate::bdos::process::request_terminate`] and the kernel SVC handler
// performs the redirect after dispatch.
// ---------------------------------------------------------------------------

/// ASCII ETX — Ctrl-C. EmuTOS `terminate()` exit code is `-32`.
const CTRL_C: u8 = 0x03;
/// Exit code EmuTOS passes to `xterm` on a console Ctrl-C
/// (`bdos/console.c:101`, `#define terminate() xterm(-32)`). The `Pexec`
/// caller receives it as `pterm::parent_exit_code(CTRLC_RC)` = 65504 (PROC06).
pub const CTRLC_RC: i32 = -32;
/// ASCII DC2 — Ctrl-R. `cgets` redisplays the current line
/// (`bdos/console.c:571`).
const CTRL_R: u8 = 0x12;
/// ASCII NAK — Ctrl-U. `cgets` kills the line, `retlen = 0`
/// (`bdos/console.c:566`).
const CTRL_U: u8 = 0x15;
/// ASCII CAN — Ctrl-X. `cgets` erases the whole line
/// (`bdos/console.c:560`).
const CTRL_X: u8 = 0x18;

// ---------------------------------------------------------------------------
// 0x01 Cconin — read one char from the console, **with echo**, Ctrl-C
// terminates. EmuTOS: bdos/console.c:386 `xconin()`.
// Spec: docs/api/gemdos/console/01-cconin.md
// ---------------------------------------------------------------------------

/// `Cconin()` — blocking read of one character with echo. Ctrl-C
/// terminates the calling process (`Pterm(-32)`). Returns the packed
/// console LONG (scancode in bits 16-23, ASCII in bits 0-7); non-ASCII
/// keys echo nothing (their ASCII byte is 0).
///
/// The device is stdin's, `HXFORM(run->p_uft[0])` (`bdos/console.c:401`,
/// [`std_dev`]), and the echo goes to that same device (`conin(h)`).
pub fn cconin(_: &TrapArgs) -> Long {
    let dev = std_dev(STDIN);
    // `Bconin` of that entry (CON: the packed key LONG), or `0` after a
    // `kbdvec` restart request (KEYS02) — ASCII 0, so nothing below happens
    // and the trap handler re-executes the call.
    let key = chardev::bconin_handle(dev);
    let ascii = (key & 0xFF) as u8;
    if ascii == CTRL_C {
        crate::bdos::process::request_terminate(CTRLC_RC);
        return 0; // value ignored — handler redirects to Pterm.
    }
    if ascii != 0 {
        chardev::bconout_handle(dev, ascii); // echo (conin → conout)
    }
    key
}

// ---------------------------------------------------------------------------
// 0x07 Crawcin — raw console input: blocking, **no echo, no Ctrl-C**.
// EmuTOS: bdos/console.c:367 `xrawcin()`.
// Spec: docs/api/gemdos/console/07-crawcin.md
// ---------------------------------------------------------------------------

/// `Crawcin()` — blocking raw read of one character. No echo, no
/// Ctrl-C handling (a Ctrl-C byte is returned as `0x03`). Returns the
/// packed console LONG (scancode in bits 16-23, ASCII in bits 0-7).
///
/// The device is stdin's, `HXFORM(run->p_uft[0])` (`bdos/console.c:377`).
pub fn crawcin(_: &TrapArgs) -> Long {
    // `Bconin` of that entry (restart request → 0, discarded).
    chardev::bconin_handle(std_dev(STDIN))
}

// ---------------------------------------------------------------------------
// 0x08 Cnecin — read one char, **no echo**, Ctrl-C terminates.
// EmuTOS: bdos/console.c:411 `xnecin()`.
// Spec: docs/api/gemdos/console/08-cnecin.md
// ---------------------------------------------------------------------------

/// `Cnecin()` — blocking read of one character without echo. Ctrl-C
/// terminates the calling process (`Pterm(-32)`). Returns the packed
/// console LONG (scancode in bits 16-23, ASCII in bits 0-7).
///
/// The device is stdin's, `HXFORM(run->p_uft[0])` (`bdos/console.c:419`).
pub fn cnecin(_: &TrapArgs) -> Long {
    // `Bconin` of that entry (restart request → 0, discarded).
    let key = chardev::bconin_handle(std_dev(STDIN));
    if (key & 0xFF) as u8 == CTRL_C {
        crate::bdos::process::request_terminate(CTRLC_RC);
        return 0; // value ignored — handler redirects to Pterm.
    }
    key
}

// ---------------------------------------------------------------------------
// 0x0A Cconrs — read an edited line into a caller buffer.
// EmuTOS: bdos/console.c:519 `xconrs(char *p)` → `cgets()`.
// Spec: docs/api/gemdos/console/0a-cconrs.md
// ---------------------------------------------------------------------------

/// Byte-level I/O backend for the [`cconrs`] line editor, abstracted out
/// of the trap path so the editing logic ([`edit_line`]) can be exercised
/// by a QEMU/EL1 selftest against a scripted input + captured echo,
/// without a live console or user-memory access.
///
/// The trap wrapper implements it over the blocking console read, the
/// console byte writer, and the caller's length-prefixed user record
/// (unprivileged `ldtrb`/`sttrb`); a selftest implements it over an
/// in-memory input script and buffer (see `kernel::selftest_console`).
pub trait LineEditIo {
    /// Blocking read of the next input byte from the edit device.
    fn read_byte(&mut self) -> u8;
    /// Echo one byte to the edit device's output.
    fn echo_byte(&mut self, b: u8);
    /// Store a data byte at buffer position `i` (0-based within the data
    /// area, i.e. the record's `p[2 + i]`). `i < max` is guaranteed.
    fn store_byte(&mut self, i: usize, b: u8);
    /// Read back a data byte previously stored at position `i` — used by
    /// the Ctrl-R redisplay to re-echo the accumulated line.
    fn load_byte(&self, i: usize) -> u8;
    /// `true` once a [`Self::store_byte`]/[`Self::load_byte`] could not reach
    /// the buffer (ARCH03 3a: the caller's record is not writable/readable
    /// for it). [`edit_line`] then stops with [`LineEdit::Faulted`]. A
    /// backend over memory that cannot fault keeps the default `false`.
    fn faulted(&self) -> bool {
        false
    }
}

/// Outcome of [`edit_line`].
pub enum LineEdit {
    /// The user finished the line (CR/LF) or filled the buffer; the value
    /// is the number of data bytes stored (the record's `p[1]`).
    Done(usize),
    /// The user pressed Ctrl-C; the caller must terminate the process
    /// (`Pterm(-32)`) and must **not** write the length byte.
    Terminated,
    /// The record could not be written (or read back): the buffer access
    /// faulted ([`LineEditIo::faulted`]). EmuTOS takes the bus error at the
    /// first such byte; the caller stops the call there.
    Faulted,
}

/// Erase one echoed character position: `BS SP BS`. NewTOS assumes a
/// one-column echo per character (no `glbcolumn` tracking, serial/fbcon
/// profile), so this is the erase width for every printable byte — the
/// same simplification the plain Backspace already uses. See the
/// divergence note on [`edit_line`].
fn erase_one<Io: LineEditIo>(io: &mut Io) {
    io.echo_byte(0x08);
    io.echo_byte(b' ');
    io.echo_byte(0x08);
}

/// The `Cconrs`/`cgets` line editor (`bdos/console.c:540`), factored to
/// run over any [`LineEditIo`]. Reads and echoes bytes until CR/LF or the
/// buffer fills, honouring the EmuTOS editing keys:
///
/// | Key                    | Action |
/// |------------------------|--------|
/// | CR (0x0D) / LF (0x0A)  | echo CR, finish, return `Done(len)`. |
/// | BS (0x08) / RUB (0x7F) | erase the last character. |
/// | Ctrl-C (0x03)          | return `Terminated`. |
/// | Ctrl-X (0x18)          | erase the whole line (`len = 0`). |
/// | Ctrl-U (0x15)          | kill the line (`len = 0`). |
/// | Ctrl-R (0x12)          | CR/LF, redisplay the buffer; `len` kept. |
/// | any other byte         | store in `buf[len++]`, echo it. |
///
/// **Divergence from EmuTOS echo (spec `0a-cconrs.md:72-73`, documented
/// low-priority):** EmuTOS renders Ctrl-U / Ctrl-R by echoing `#` then a
/// `newline(h, stcol)` that re-indents to the captured *starting column*.
/// NewTOS keeps no per-device column (`glbcolumn`), so it cannot re-indent:
/// Ctrl-U instead takes the echoed characters back in place with `BS SP BS`
/// (identical to Ctrl-X in this model — both reduce to `len = 0`), and
/// Ctrl-R re-shows the line with a plain `CR LF` before re-echoing. The
/// **observable buffer contract matches EmuTOS exactly**: Ctrl-U/Ctrl-X
/// leave `len = 0` (subsequent input starts at `p[1] = 0`), Ctrl-R leaves
/// `len` unchanged. The `cookdout` `^X` control-echo and tab expansion of
/// full `cgets` are likewise not modelled (see F5/F6 in the console audit).
pub fn edit_line<Io: LineEditIo>(io: &mut Io, max: usize) -> LineEdit {
    let mut len: usize = 0;
    while len < max {
        let b = io.read_byte();
        match b {
            0x0D | 0x0A => {
                io.echo_byte(0x0D); // echo CR only (no LF), per cgets
                return LineEdit::Done(len);
            }
            CTRL_C => return LineEdit::Terminated,
            0x08 | 0x7F => {
                if len > 0 {
                    len -= 1;
                    erase_one(io);
                }
            }
            // Ctrl-X (erase line) and Ctrl-U (kill line) both reduce to
            // `len = 0`; with no column tracking their echo collapses to
            // the same in-place `BS SP BS` take-back (see divergence note).
            CTRL_X | CTRL_U => {
                while len > 0 {
                    len -= 1;
                    erase_one(io);
                }
            }
            CTRL_R => {
                // Redisplay: fresh line, then re-echo the kept buffer.
                io.echo_byte(0x0D);
                io.echo_byte(0x0A);
                for i in 0..len {
                    let c = io.load_byte(i);
                    if io.faulted() {
                        return LineEdit::Faulted;
                    }
                    io.echo_byte(c);
                }
            }
            _ => {
                io.store_byte(len, b);
                if io.faulted() {
                    return LineEdit::Faulted;
                }
                len += 1;
                io.echo_byte(b); // echo
            }
        }
    }
    LineEdit::Done(len)
}

/// [`LineEditIo`] over the trap path: the installed console host + the
/// caller's record pointer `p` (`[max, len, buf…]`). Buffer bytes are read
/// and written in the caller's space (ARCH03 3a, [`crate::uaccess`]): an EL0
/// program's record through the fault-recovered unprivileged byte accesses,
/// permission-checked as EL0 — the `Cconrs` contract that `p` is a user
/// pointer — an in-kernel caller's as the kernel address it is.
struct TrapLineIo {
    host: &'static dyn crate::host::Host,
    /// Whose address space `p` is in.
    space: crate::uaccess::Space,
    /// The caller's length-prefixed record pointer (`p`).
    p: u64,
    /// A buffer access faulted ([`LineEditIo::faulted`]).
    faulted: core::cell::Cell<bool>,
}

impl LineEditIo for TrapLineIo {
    fn read_byte(&mut self) -> u8 {
        self.host.console_read_byte_blocking()
    }
    fn echo_byte(&mut self, b: u8) {
        self.host.console_write_byte(b);
    }
    fn store_byte(&mut self, i: usize, b: u8) {
        // `i < max`, so `p + 2 + i` is within the record's buffer area.
        if self.space.put_u8(self.p + 2 + i as u64, b).is_err() {
            self.faulted.set(true);
        }
    }
    fn load_byte(&self, i: usize) -> u8 {
        // `i < len <= max`: a byte this call stored before.
        self.space.get_u8(self.p + 2 + i as u64).unwrap_or_else(|_| {
            self.faulted.set(true);
            0
        })
    }
    fn faulted(&self) -> bool {
        self.faulted.get()
    }
}

/// `Cconrs(p)` — read an edited line into the caller's record.
///
/// `p[0]` (IN) = max length, `p[1]` (OUT) = actual length, `p[2..]` =
/// buffer. Blocking, with the [`edit_line`] line editor: CR/LF finishes
/// (echo CR), BS/DEL erases one char, Ctrl-X / Ctrl-U kill the line,
/// Ctrl-R redisplays it, Ctrl-C terminates (`Pterm(-32)`), other bytes
/// are stored + echoed. The buffer is **not** NUL-terminated — callers
/// use `p[1]` as the length (TOS contract). Returns `0` (`xconrs` is
/// `void`). See [`edit_line`] for the EmuTOS echo divergence.
///
/// The device is stdin's, `HXFORM(run->p_uft[0])` (`bdos/console.c:535`,
/// [`std_dev`]): CON reads and echoes through the console host
/// ([`TrapLineIo`]); any other device runs the same editor over that table
/// entry (`cgets(h, …)`, as `Fread` on a device does).
pub fn cconrs(args: &TrapArgs) -> Long {
    let h = host_or_eundev!();
    let p = args.arg_u64(0);
    if p == 0 {
        return 0;
    }
    // The record is in the caller's space (`TrapLineIo`); a record the
    // program cannot read or write ends the call at the first such byte and
    // — through the syscall tail — the program (EmuTOS: bus error in
    // `cgets`, `docs/api/bios/_exception-crash.md`).
    let Ok(max) = args.space.get_u8(p) else {
        return crate::uaccess::FAULTED;
    };
    let dev = std_dev(STDIN);
    let outcome = if dev == DEV_CON {
        let mut io = TrapLineIo { host: h, space: args.space, p, faulted: core::cell::Cell::new(false) };
        edit_line(&mut io, max as usize)
    } else {
        // A BIOS handle with no table entry (`p_uft[0]` below `H_Print`):
        // EmuTOS' `Bconin`/`Bconout` answer 0 there (`bios/bios.c:973-976,
        // 1008-1010`), so `cgets` stores NULs until `max` — the absent
        // device's behaviour.
        let d = chardev::device(dev).unwrap_or(&chardev::Absent);
        let buf = SpaceBuf { space: args.space, base: p + 2 };
        let mut io = DeviceLineIo { dev: d, buf: &buf, faulted: core::cell::Cell::new(false) };
        edit_line(&mut io, max as usize)
    };
    let len = match outcome {
        LineEdit::Done(n) => n,
        LineEdit::Terminated => {
            crate::bdos::process::request_terminate(CTRLC_RC);
            return 0; // value ignored — handler redirects to Pterm.
        }
        LineEdit::Faulted => return crate::uaccess::FAULTED,
    };
    // `p + 1` is the record's length byte.
    if args.space.put_u8(p + 1, len as u8).is_err() {
        return crate::uaccess::FAULTED;
    }
    0
}

// ---------------------------------------------------------------------------
// 0x09 Cconws — write a NUL-terminated string.
// EmuTOS: bdos/console.c:96  `xconws(char *s)`.
// Returns the number of bytes written (LONG).
// Spec: docs/api/gemdos/console/09-cconws.md
// ---------------------------------------------------------------------------

/// `Cconws(s)` — write zero-terminated string `s` to the console.
/// Returns the byte count actually written.
///
/// EmuTOS' `prt_line` writes each byte via `conout`, which runs `conbrk`
/// first; NewTOS mirrors that with the Ctrl-C-only [`output_break`] before
/// each byte — a pending Ctrl-C stops the write early (the rest of the
/// string is not emitted) and defers `Pterm(-32)`.
///
/// The device is stdout's, `HXFORM(run->p_uft[1])` (`bdos/console.c:461`,
/// [`std_dev`]); on a device other than CON there is no break check, as in
/// [`cconout`].
///
/// # User pointer
///
/// `s` is read in the caller's space ([`crate::trap::TrapArgs::space`]):
/// an EL0 program's pointer with fault-safe unprivileged loads
/// ([`crate::uaccess`]), an in-kernel caller's as the kernel address it is.
/// A pointer the program cannot read ends the call and — through the
/// kernel's syscall tail — the caller (EmuTOS: bus error in the trap → `dopanic` →
/// `Pterm(-1)`, `docs/api/bios/_exception-crash.md`).
pub fn cconws(args: &TrapArgs) -> Long {
    let mut p = args.arg_ptr(0);
    if p.is_null() {
        return 0;
    }
    let h = host_or_eundev!();
    let dev = std_dev(STDOUT);
    let mut count: Long = 0;
    // Hard cap to prevent runaway loops if the caller forgot the NUL.
    // 64 KiB matches TOS's effective practical limit (Cconrs buffers).
    const MAX: Long = 65_535;
    while count < MAX {
        // `s` is a *user* pointer: read through `uaccess` (unprivileged
        // `ldtrb`, permission-checked as EL0 — the access the trap contract
        // calls for). A bad pointer no longer halts the machine (ARCH03 0c):
        // the read faults recoverably, the bytes before it are already
        // written (as on EmuTOS, whose `prt_line` takes the bus error in the
        // middle of the string), and the syscall tail terminates the caller
        // with the crash report — the count returned here never reaches it.
        let Ok(b) = args.space.get_u8(p as u64) else {
            break;
        };
        if b == 0 {
            break;
        }
        // conbrk before each output byte (Ctrl-C aborts the string).
        if dev == DEV_CON && output_break(h) {
            break; // Ctrl-C: stop writing; SVC handler redirects to Pterm.
        }
        chardev::bconout_handle(dev, b);
        // SAFETY: same provenance as `p`; bounded by `MAX` above.
        p = unsafe { p.add(1) };
        count += 1;
    }
    count
}

// ---------------------------------------------------------------------------
// 0x0B Cconis — non-blocking console-input status.
// EmuTOS: bdos/console.c:108 `xconstat()` → `Bconstat(2)`.
// Returns -1 if input ready, 0 otherwise. (LONG, but spec uses the
// "all-bits-set" form which is sign-extended.)
// ---------------------------------------------------------------------------

/// `Cconis()` — `constat(HXFORM(run->p_uft[0]))` (`bdos/console.c:161`):
/// by default `Bconstat(2)` of the CON entry, `-1` if a console byte is
/// available, else `0` (NewTOS has no BDOS typeahead buffer to check first);
/// a stdin device other than AUX/CON answers `0` ([`constat`]).
pub fn cconis(_: &TrapArgs) -> Long {
    constat(std_dev(STDIN))
}

// ---------------------------------------------------------------------------
// 0x10 Cconos — console-output status. EmuTOS routes to Bcostat(2).
// ---------------------------------------------------------------------------

/// `Cconos()` — `Bcostat(HXFORM(run->p_uft[1]))` (`bdos/console.c:170`): by
/// default the CON entry, `-1` (the console is always ready to accept a
/// byte).
pub fn cconos(_: &TrapArgs) -> Long {
    chardev::bcostat_handle(std_dev(STDOUT))
}

// ---------------------------------------------------------------------------
// 0x11 Cprnos — printer output status = Bcostat(0).
// Spec: docs/api/gemdos/console/11-cprnos.md (FS11 note).
// ---------------------------------------------------------------------------

/// `Cprnos()` — `Bcostat(HXFORM(run->p_uft[3]))` (`bdos/console.c:179`): by
/// default the PRN entry; `0` (not ready) for the absent printer, as EmuTOS
/// without `CONF_WITH_PRINTER_PORT` (`bios/parport.c:139-140`).
pub fn cprnos(_: &TrapArgs) -> Long {
    chardev::bcostat_handle(std_dev(STDPRN))
}

// ---------------------------------------------------------------------------
// 0x12 Cauxis — AUX input status = constat(1) → Bconstat(1).
// Spec: docs/api/gemdos/console/12-cauxis.md (FS11 note).
// ---------------------------------------------------------------------------

/// `Cauxis()` — `constat(HXFORM(run->p_uft[2]))` (`bdos/console.c:188`): by
/// default `Bconstat(1)` of the AUX entry (no BDOS typeahead buffer to check
/// first); `0` — the no-cable AUX entry never has input. A stdaux device
/// other than AUX/CON answers `0` ([`constat`]).
pub fn cauxis(_: &TrapArgs) -> Long {
    constat(std_dev(STDAUX))
}

// ---------------------------------------------------------------------------
// 0x13 Cauxos — AUX output status = Bcostat(1).
// Spec: docs/api/gemdos/console/13-cauxos.md (FS11 note).
// ---------------------------------------------------------------------------

/// `Cauxos()` — `Bcostat(HXFORM(run->p_uft[2]))` (`bdos/console.c:197`): by
/// default the AUX entry, `-1` (ready) for the no-cable entry
/// (`docs/api/gemdos/console/13-cauxos.md`, option A).
pub fn cauxos(_: &TrapArgs) -> Long {
    chardev::bcostat_handle(std_dev(STDAUX))
}

// ---------------------------------------------------------------------------
// Fread / Fwrite on a character-device handle (FS11).
// EmuTOS: bdos/bdosmain.c:604-640 (`osif` device block) with `conin`
// (bdos/console.c:384-390), `cgets` (:540-581) and `tabout` (:292-302).
// Spec: docs/api/gemdos/file/{3f-fread,40-fwrite}.md "Character-device path"
// and "NewTOS implementation notes".
// ---------------------------------------------------------------------------

/// BDOS handle of the console (`H_Console`, `bdos/console.h`).
const H_CONSOLE: i16 = -1;

/// BDOS character-device handle (`-1` CON, `-2` AUX, `-3` PRN) → BIOS device
/// number: EmuTOS `HXFORM(h) = 3 + h` (`bdos/console.h:36`).
fn hxform(numl: i16) -> u16 {
    (3 + numl) as u16
}

/// Byte access to the caller's buffer for the device paths: an index into
/// the buffer, `None` / `false` when that byte cannot be reached (ARCH03 3a).
trait DevBuf {
    fn get(&self, i: u64) -> Option<u8>;
    fn put(&self, i: u64, b: u8) -> bool;
}

/// [`DevBuf`] over the caller's address space.
struct SpaceBuf {
    space: crate::uaccess::Space,
    base: u64,
}

impl DevBuf for SpaceBuf {
    fn get(&self, i: u64) -> Option<u8> {
        self.space.get_u8(self.base + i).ok()
    }
    fn put(&self, i: u64, b: u8) -> bool {
        self.space.put_u8(self.base + i, b).is_ok()
    }
}

/// [`LineEditIo`] over one device-table entry and the caller's buffer — the
/// `cgets(h, …)` of `Fread`: input from that device's `Bconin` (its low byte,
/// `char ch`, `console.c:542`), echo to that device's `Bconout`.
struct DeviceLineIo<'a, B: DevBuf> {
    dev: &'static dyn CharDevice,
    buf: &'a B,
    faulted: core::cell::Cell<bool>,
}

impl<B: DevBuf> LineEditIo for DeviceLineIo<'_, B> {
    fn read_byte(&mut self) -> u8 {
        // Never gives way to a `kbdvec` hook: keys may already be stored
        // (the same blocking read `Cconrs` uses).
        self.dev.bconin() as u8
    }
    fn echo_byte(&mut self, b: u8) {
        let _ = self.dev.bconout(b); // `conout` ignores the BIOS result
    }
    fn store_byte(&mut self, i: usize, b: u8) {
        if !self.buf.put(i as u64, b) {
            self.faulted.set(true);
        }
    }
    fn load_byte(&self, i: usize) -> u8 {
        self.buf.get(i as u64).unwrap_or_else(|| {
            self.faulted.set(true);
            0
        })
    }
    fn faulted(&self) -> bool {
        self.faulted.get()
    }
}

/// `Fread` on a character-device handle `numl` (`-1..-3`): `len` bytes into
/// `ubuf` in `space` (EmuTOS `osif`, `bdos/bdosmain.c:608-620`).
pub(crate) fn device_read(numl: i16, space: crate::uaccess::Space, len: i32, ubuf: u64) -> Long {
    device_read_with(numl, len, &SpaceBuf { space, base: ubuf })
}

fn device_read_with<B: DevBuf>(numl: i16, len: i32, buf: &B) -> Long {
    let dev = hxform(numl);
    let Some(d) = chardev::device(dev) else {
        return 0;
    };
    // "HUGE read": the high word of the LONG is tested (`pw[2]`), which also
    // catches every negative `len`.
    if (len as u32) >> 16 != 0 {
        return 0;
    }
    // `pw[3]` is a signed WORD: 32768..65535 are negative here.
    let lo = len as u16 as i16;
    if lo == 1 {
        // `conin(dev)`: one key, echoed, stored as the low byte of the
        // `Bconin` LONG — CR and Ctrl-C included, no line editing. No side
        // effect happens before the key arrives, so the read may give way to
        // a `kbdvec` hook (KEYS02) and be re-executed.
        let Some(key) = d.bconin_restartable() else {
            crate::xbios::kbdvecs::request_restart();
            return 0; // discarded — the trap handler re-executes the call
        };
        let ch = key as u8;
        let _ = d.bconout(ch);
        if !buf.put(0, ch) {
            return crate::uaccess::FAULTED;
        }
        return 1;
    }
    if lo <= 0 {
        // `cgets` with `maxlen <= 0`: the loop never runs (`console.c:546`).
        return 0;
    }
    // `cgets(dev, len, buf)` = the `Cconrs` line editor over this device.
    let mut io = DeviceLineIo { dev: d, buf, faulted: core::cell::Cell::new(false) };
    match edit_line(&mut io, lo as usize) {
        LineEdit::Done(n) => n as Long,
        LineEdit::Terminated => {
            crate::bdos::process::request_terminate(CTRLC_RC);
            0 // value ignored — handler redirects to Pterm.
        }
        LineEdit::Faulted => crate::uaccess::FAULTED,
    }
}

/// `Fwrite` on a character-device handle `numl` (`-1..-3`): `count` bytes
/// from `ubuf` in `space` (EmuTOS `osif`, `bdos/bdosmain.c:622-640`).
///
/// `count <= 0` returns `count` itself. Each byte is read from the buffer
/// first (an unreadable byte → [`crate::uaccess::FAULTED`], the bytes before
/// it stay written). CON: the Ctrl-C-only [`output_break`], then the raw byte
/// (`Cconws` behaviour, the approved divergence from `tabout`), result
/// `count`. Any other device: the raw byte to its `Bconout`, no break check;
/// a `0` from `Bconout` ends the call with the number of bytes before it.
pub(crate) fn device_write(numl: i16, space: crate::uaccess::Space, count: i32, ubuf: u64) -> Long {
    device_write_with(numl, count, &SpaceBuf { space, base: ubuf })
}

fn device_write_with<B: DevBuf>(numl: i16, count: i32, buf: &B) -> Long {
    if count <= 0 {
        return count;
    }
    let Some(d) = chardev::device(hxform(numl)) else {
        return 0;
    };
    for n in 0..count as u64 {
        let Some(b) = buf.get(n) else {
            return crate::uaccess::FAULTED;
        };
        if numl == H_CONSOLE {
            if host().is_some_and(output_break) {
                return n as Long; // Ctrl-C: SVC handler redirects to Pterm.
            }
            let _ = d.bconout(b); // `tabout` is void
        } else if d.bconout(b) == 0 {
            return n as Long;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scripted line-editor backend whose buffer becomes unreachable from
    /// byte `fault_at` on (ARCH03 3a: the caller's record is not writable).
    struct FaultingIo {
        script: &'static [u8],
        at: usize,
        buf: [u8; 16],
        fault_at: usize,
        faulted: bool,
        echoed: usize,
    }

    impl LineEditIo for FaultingIo {
        fn read_byte(&mut self) -> u8 {
            let b = self.script[self.at];
            self.at += 1;
            b
        }
        fn echo_byte(&mut self, _b: u8) {
            self.echoed += 1;
        }
        fn store_byte(&mut self, i: usize, b: u8) {
            if i >= self.fault_at {
                self.faulted = true;
            } else {
                self.buf[i] = b;
            }
        }
        fn load_byte(&self, i: usize) -> u8 {
            self.buf[i]
        }
        fn faulted(&self) -> bool {
            self.faulted
        }
    }

    #[test]
    fn edit_line_stops_at_the_first_store_that_faults() {
        let mut io = FaultingIo { script: b"abcd\r", at: 0, buf: [0; 16], fault_at: 2, faulted: false, echoed: 0 };
        assert!(matches!(edit_line(&mut io, 10), LineEdit::Faulted));
        // 'a' and 'b' were stored and echoed; 'c' faulted and was not echoed,
        // and no further key was read.
        assert_eq!(&io.buf[..2], b"ab");
        assert_eq!(io.echoed, 2);
        assert_eq!(io.at, 3);
    }

    /// A device-path buffer whose bytes from `fault_at` on are unreachable.
    struct FaultBuf {
        data: [core::cell::Cell<u8>; 8],
        fault_at: u64,
    }

    impl FaultBuf {
        fn new(fault_at: u64) -> FaultBuf {
            FaultBuf { data: Default::default(), fault_at }
        }
    }

    impl DevBuf for FaultBuf {
        fn get(&self, i: u64) -> Option<u8> {
            (i < self.fault_at).then(|| self.data[i as usize].get())
        }
        fn put(&self, i: u64, b: u8) -> bool {
            if i >= self.fault_at {
                return false;
            }
            self.data[i as usize].set(b);
            true
        }
    }

    /// WT4 (`40-fwrite.md`): the byte is read from the user buffer before the
    /// device call — an unreadable byte 0 is `FAULTED`, not the device's
    /// result (the absent PRN's short count 0, the no-cable AUX's count).
    #[test]
    fn wt4_fwrite_reads_the_byte_before_the_device_call() {
        for numl in [-2, -3] {
            assert_eq!(device_write_with(numl, 3, &FaultBuf::new(0)), crate::uaccess::FAULTED);
        }
        // Readable: the absent PRN refuses the first byte → short count 0;
        // the no-cable AUX accepts every byte → 3 (`3f-fread.md` item 1a).
        assert_eq!(device_write_with(-3, 3, &FaultBuf::new(8)), 0);
        assert_eq!(device_write_with(-2, 3, &FaultBuf::new(8)), 3);
    }

    /// The device read stops with `FAULTED` at the first byte it cannot store
    /// (ARCH03 3a, `3f-fread.md` implementation notes).
    #[test]
    fn fread_device_store_fault() {
        assert_eq!(device_read_with(-2, 1, &FaultBuf::new(0)), crate::uaccess::FAULTED);
        let b = FaultBuf::new(2);
        assert_eq!(device_read_with(-3, 4, &b), crate::uaccess::FAULTED);
        assert_eq!(device_read_with(-3, 2, &FaultBuf::new(2)), 2);
    }

    #[test]
    fn edit_line_without_faults_is_unchanged() {
        let mut io = FaultingIo { script: b"ab\x08c\r", at: 0, buf: [0; 16], fault_at: 16, faulted: false, echoed: 0 };
        assert!(matches!(edit_line(&mut io, 10), LineEdit::Done(2)));
        assert_eq!(&io.buf[..2], b"ac");
    }
}
