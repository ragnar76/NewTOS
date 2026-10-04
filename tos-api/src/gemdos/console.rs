//! GEMDOS console group — output-side leaf functions (PHASE-02 D3).
//!
//! Specs: `docs/api/gemdos/console/*.md`.
//!
//! All functions take a borrow of [`TrapArgs`] and return a `Long`
//! (`i32` in `x0`). The semantics mirror EmuTOS exactly so binary
//! programs see TOS-bit-compatible behaviour; per-function citations
//! point to the EmuTOS source location used to derive the code.

use crate::bios::chardev::pack_console_key;
use crate::host::host;
use crate::trap::TrapArgs;
use tos_types::error::{EUNDEV, E_OK};
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
fn output_break(h: &dyn crate::host::Host) -> bool {
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
/// EmuTOS' `xconout` runs `conbrk` before writing; NewTOS mirrors that
/// with the Ctrl-C-only [`output_break`] — a pending Ctrl-C aborts the
/// write and defers `Pterm(-32)`.
pub fn cconout(args: &TrapArgs) -> Long {
    let ch = args.arg_u16(0) as u8;
    let h = host_or_eundev!();
    if output_break(h) {
        return 1; // Ctrl-C: aborted; SVC handler redirects to Pterm.
    }
    h.console_write_byte(ch);
    1
}

// ---------------------------------------------------------------------------
// AUX BIOS handle. EmuTOS' `xauxin`/`xauxout` resolve stdaux from
// `run->p_uft[2]` (default `H_Aux = -2`) and call `Bconin(HXFORM(-2))` /
// `Bconout(HXFORM(-2), ch)` where `HXFORM(h) = 3 + h`, i.e. BIOS handle 1
// (`docs/api/gemdos/console/{03-cauxin,04-cauxout}.md`).
// ---------------------------------------------------------------------------
const AUX_BIOS_HANDLE: u16 = 1;

// ---------------------------------------------------------------------------
// 0x03 Cauxin — read a single byte from AUX.
// EmuTOS: bdos/console.c:432 `xauxin()` = `Bconin(1)` — a pure pass-through
// (docs/api/gemdos/console/03-cauxin.md).
// ---------------------------------------------------------------------------

/// `Cauxin()` — pass-through to `Bconin(1)`, exactly as EmuTOS' `xauxin`.
/// The NewTOS profile has no separate AUX device: BIOS handle 1 is an
/// absent (`char_dummy`) slot, so the pass-through returns `0` (no AUX
/// byte) instead of blocking on serial hardware that does not exist. Not
/// a stub — it routes through the same BIOS input path a real AUX device
/// would use; the value falls out of the absent-device contract.
pub fn cauxin(_: &TrapArgs) -> Long {
    crate::bios::chardev::bconin_handle(AUX_BIOS_HANDLE)
}

// ---------------------------------------------------------------------------
// 0x04 Cauxout — write a single byte to AUX.
// EmuTOS: bdos/console.c:331 `xauxout(ch)` = `Bconout(1, ch)` — a pure
// pass-through (docs/api/gemdos/console/04-cauxout.md).
// ---------------------------------------------------------------------------

/// `Cauxout(ch)` — pass-through to `Bconout(1, ch)`, exactly as EmuTOS'
/// `xauxout`. BIOS handle 1 (AUX) is an absent (`char_dummy`) slot in the
/// NewTOS profile, so the byte is discarded and the call returns `0`. Not
/// a stub — it routes through the same BIOS output path a real AUX device
/// would use.
pub fn cauxout(args: &TrapArgs) -> Long {
    let ch = args.arg_u16(0) as u8;
    crate::bios::chardev::bconout_handle(AUX_BIOS_HANDLE, ch)
}

// ---------------------------------------------------------------------------
// 0x05 Cprnout — write a single byte to the printer.
// EmuTOS: bdos/console.c:340 `xprtout(WORD ch)` = `Bconout(0, ch)`.
// NewTOS profile: no printer, so the parport `bconout0` equivalent times
// out / is absent and returns `0` — which is exactly what the spec
// requires (docs/api/gemdos/console/05-cprnout.md:87-89: `0` = printer
// busy/absent, a non-negative ready/busy signal, *not* a TOS error code).
// ---------------------------------------------------------------------------

/// `Cprnout(ch)` — printer output. NewTOS has no printer port, so the byte
/// is never accepted and the call returns `0` (the printer-busy/absent
/// ready signal, per `docs/api/gemdos/console/05-cprnout.md`), the same
/// value EmuTOS' `bconout0` returns on timeout / with the port disabled.
pub fn cprnout(_: &TrapArgs) -> Long {
    0
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
pub fn crawio(args: &TrapArgs) -> Long {
    let w = args.arg_u16(0);
    let h = host_or_eundev!();
    if w == 0x00FF {
        // Read side: non-blocking; return 0 if no input. Uses the whole
        // key word so a pending non-ASCII key (arrow / function) is not
        // silently dropped.
        match h.console_try_read_key() {
            Some(word) => pack_console_key(word),
            None => 0,
        }
    } else {
        h.console_write_byte(w as u8);
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
/// Exit code EmuTOS passes to `xterm` on a console Ctrl-C.
const CTRLC_RC: i32 = -32;
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
pub fn cconin(_: &TrapArgs) -> Long {
    let h = host_or_eundev!();
    let Some(word) = h.console_read_key_restartable() else {
        // A `kbdvec` hook has bytes waiting: restart after it ran (KEYS02).
        crate::xbios::kbdvecs::request_restart();
        return 0; // discarded — the trap handler re-executes the call
    };
    let ascii = (word & 0xFF) as u8;
    if ascii == CTRL_C {
        crate::bdos::process::request_terminate(CTRLC_RC);
        return 0; // value ignored — handler redirects to Pterm.
    }
    if ascii != 0 {
        h.console_write_byte(ascii); // echo (conin → conout)
    }
    pack_console_key(word)
}

// ---------------------------------------------------------------------------
// 0x07 Crawcin — raw console input: blocking, **no echo, no Ctrl-C**.
// EmuTOS: bdos/console.c:367 `xrawcin()`.
// Spec: docs/api/gemdos/console/07-crawcin.md
// ---------------------------------------------------------------------------

/// `Crawcin()` — blocking raw read of one character. No echo, no
/// Ctrl-C handling (a Ctrl-C byte is returned as `0x03`). Returns the
/// packed console LONG (scancode in bits 16-23, ASCII in bits 0-7).
pub fn crawcin(_: &TrapArgs) -> Long {
    let Some(word) = host_or_eundev!().console_read_key_restartable() else {
        // A `kbdvec` hook has bytes waiting: restart after it ran (KEYS02).
        crate::xbios::kbdvecs::request_restart();
        return 0; // discarded — the trap handler re-executes the call
    };
    pack_console_key(word)
}

// ---------------------------------------------------------------------------
// 0x08 Cnecin — read one char, **no echo**, Ctrl-C terminates.
// EmuTOS: bdos/console.c:411 `xnecin()`.
// Spec: docs/api/gemdos/console/08-cnecin.md
// ---------------------------------------------------------------------------

/// `Cnecin()` — blocking read of one character without echo. Ctrl-C
/// terminates the calling process (`Pterm(-32)`). Returns the packed
/// console LONG (scancode in bits 16-23, ASCII in bits 0-7).
pub fn cnecin(_: &TrapArgs) -> Long {
    let Some(word) = host_or_eundev!().console_read_key_restartable() else {
        // A `kbdvec` hook has bytes waiting: restart after it ran (KEYS02).
        crate::xbios::kbdvecs::request_restart();
        return 0; // discarded — the trap handler re-executes the call
    };
    if (word & 0xFF) as u8 == CTRL_C {
        crate::bdos::process::request_terminate(CTRLC_RC);
        return 0; // value ignored — handler redirects to Pterm.
    }
    pack_console_key(word)
}

// ---------------------------------------------------------------------------
// 0x0A Cconrs — read an edited line into a caller buffer.
// EmuTOS: bdos/console.c:519 `xconrs(char *p)` → `cgets()`.
// Spec: docs/api/gemdos/console/0a-cconrs.md
// ---------------------------------------------------------------------------

/// Read one byte from a **user** pointer via an unprivileged load
/// (`ldtrb`), so the access is permission-checked as EL0 — `Cconrs`'s
/// `p` is a user pointer (same contract as [`cconws`]).
///
/// # Safety
///
/// `p` must be a readable user address.
#[inline]
unsafe fn read_user_byte(p: *const u8) -> u8 {
    let b: u8;
    // SAFETY: caller guarantees `p` readable; unprivileged byte load.
    unsafe {
        core::arch::asm!(
            "ldtrb {b:w}, [{p}]",
            p = in(reg) p,
            b = out(reg) b,
            options(nostack, preserves_flags, readonly),
        );
    }
    b
}

/// Write one byte to a **user** pointer via an unprivileged store
/// (`sttrb`).
///
/// # Safety
///
/// `p` must be a writable user address.
#[inline]
unsafe fn write_user_byte(p: *mut u8, b: u8) {
    // SAFETY: caller guarantees `p` writable; unprivileged byte store.
    unsafe {
        core::arch::asm!(
            "sttrb {b:w}, [{p}]",
            p = in(reg) p,
            b = in(reg) b,
            options(nostack, preserves_flags),
        );
    }
}

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
}

/// Outcome of [`edit_line`].
pub enum LineEdit {
    /// The user finished the line (CR/LF) or filled the buffer; the value
    /// is the number of data bytes stored (the record's `p[1]`).
    Done(usize),
    /// The user pressed Ctrl-C; the caller must terminate the process
    /// (`Pterm(-32)`) and must **not** write the length byte.
    Terminated,
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
                    io.echo_byte(c);
                }
            }
            _ => {
                io.store_byte(len, b);
                len += 1;
                io.echo_byte(b); // echo
            }
        }
    }
    LineEdit::Done(len)
}

/// [`LineEditIo`] over the trap path: the installed console host + the
/// caller's user-space record pointer `p` (`[max, len, buf…]`). Buffer
/// bytes are read/written through unprivileged `ldtrb`/`sttrb` so the
/// access is permission-checked as EL0, matching the `Cconrs` contract
/// that `p` is a user pointer.
struct TrapLineIo {
    host: &'static dyn crate::host::Host,
    /// The caller's length-prefixed record pointer (`p`).
    p: *mut u8,
}

impl LineEditIo for TrapLineIo {
    fn read_byte(&mut self) -> u8 {
        self.host.console_read_byte_blocking()
    }
    fn echo_byte(&mut self, b: u8) {
        self.host.console_write_byte(b);
    }
    fn store_byte(&mut self, i: usize, b: u8) {
        // SAFETY: caller bounds `i < max`, so `p + 2 + i` is within the
        // record's buffer area; unprivileged store.
        unsafe { write_user_byte(self.p.add(2 + i), b) };
    }
    fn load_byte(&self, i: usize) -> u8 {
        // SAFETY: `i < len <= max`, so `p + 2 + i` was previously stored
        // within the buffer area; unprivileged load.
        unsafe { read_user_byte(self.p.add(2 + i)) }
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
pub fn cconrs(args: &TrapArgs) -> Long {
    let h = host_or_eundev!();
    let p = args.arg_ptr(0) as *mut u8;
    if p.is_null() {
        return 0;
    }
    // SAFETY: `p` is the caller-supplied user record pointer; the max
    // length byte is read through an unprivileged load.
    let max = unsafe { read_user_byte(p) } as usize;
    let mut io = TrapLineIo { host: h, p };
    let len = match edit_line(&mut io, max) {
        LineEdit::Done(n) => n,
        LineEdit::Terminated => {
            crate::bdos::process::request_terminate(CTRLC_RC);
            return 0; // value ignored — handler redirects to Pterm.
        }
    };
    // SAFETY: `p + 1` is the record's length byte.
    unsafe { write_user_byte(p.add(1), len as u8) };
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
/// # Safety in trap context
///
/// The pointer comes from the trapping caller. PHASE-02 does not yet
/// have user/kernel separation, so we trust the pointer. PHASE-06
/// will add a bounds check against the calling process's TPA.
pub fn cconws(args: &TrapArgs) -> Long {
    let mut p = args.arg_ptr(0);
    if p.is_null() {
        return 0;
    }
    let h = host_or_eundev!();
    let mut count: Long = 0;
    // Hard cap to prevent runaway loops if the caller forgot the NUL.
    // 64 KiB matches TOS's effective practical limit (Cconrs buffers).
    const MAX: Long = 65_535;
    while count < MAX {
        // SAFETY: pointer validated above; see PHASE-06 note for the
        // future kernel/user boundary check. We use the
        // unprivileged-load `ldtrb` so the access is checked as if
        // EL0 made it — this both honours the user-page permissions
        // and bypasses any PSTATE.PAN restriction the runtime
        // applies to from-kernel user-VA reads (cortex-a72 doesn't
        // implement PAN but the QEMU tcg path appears to enforce
        // similar semantics, observed during Phase-06-D3 bring-up).
        let b: u8;
        unsafe {
            core::arch::asm!(
                "ldtrb {b:w}, [{p}]",
                p = in(reg) p,
                b = out(reg) b,
                options(nostack, preserves_flags, readonly),
            );
        }
        if b == 0 {
            break;
        }
        // conbrk before each output byte (Ctrl-C aborts the string).
        if output_break(h) {
            break; // Ctrl-C: stop writing; SVC handler redirects to Pterm.
        }
        h.console_write_byte(b);
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

/// `Cconis()` — returns `-1` if a console byte is available, else `0`.
pub fn cconis(_: &TrapArgs) -> Long {
    let h = match host() {
        Some(h) => h,
        None => return E_OK,
    };
    if h.console_has_input() {
        -1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 0x10 Cconos — console-output status. EmuTOS routes to Bcostat(2).
// PL011 with FIFO is virtually always ready; return -1 unconditionally.
// ---------------------------------------------------------------------------

/// `Cconos()` — returns `-1` (output device ready).
pub fn cconos(_: &TrapArgs) -> Long {
    -1
}

// ---------------------------------------------------------------------------
// 0x11 Cprnos — printer output status. No printer → not ready → 0.
// ---------------------------------------------------------------------------

/// `Cprnos()` — returns `0` (printer not ready / not present).
pub fn cprnos(_: &TrapArgs) -> Long {
    0
}

// ---------------------------------------------------------------------------
// 0x12 Cauxis — AUX input status. No AUX in PHASE-02 → 0.
// ---------------------------------------------------------------------------

/// `Cauxis()` — returns `0` (AUX input not available).
pub fn cauxis(_: &TrapArgs) -> Long {
    0
}

// ---------------------------------------------------------------------------
// 0x13 Cauxos — AUX output status. No AUX in PHASE-02 → 0.
// ---------------------------------------------------------------------------

/// `Cauxos()` — returns `0` (AUX output not available).
pub fn cauxos(_: &TrapArgs) -> Long {
    0
}
