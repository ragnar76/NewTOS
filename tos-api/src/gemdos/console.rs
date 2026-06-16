//! GEMDOS console group — output-side leaf functions (PHASE-02 D3).
//!
//! Specs: `docs/api/gemdos/console/*.md`.
//!
//! All functions take a borrow of [`TrapArgs`] and return a `Long`
//! (`i32` in `x0`). The semantics mirror EmuTOS exactly so binary
//! programs see TOS-bit-compatible behaviour; per-function citations
//! point to the EmuTOS source location used to derive the code.

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
// 0x02 Cconout — write a single byte to the console.
// EmuTOS: bdos/console.c:78  `xconout(WORD ch)`.
// Spec:   docs/api/gemdos/console/02-cconout.md
// ---------------------------------------------------------------------------

/// `Cconout(ch)` — write `ch` (low 8 bits) to the console output.
/// Always returns `1` (per `docs/api/gemdos/console/02-cconout.md`
/// and discrepancy `D-001`: EmuTOS unconditionally returns `1`,
/// even though Compendium and Profibuch document the value as
/// "undefined" or "the function code"). NewTOS follows EmuTOS.
pub fn cconout(args: &TrapArgs) -> Long {
    let ch = args.arg_u16(0) as u8;
    host_or_eundev!().console_write_byte(ch);
    1
}

// ---------------------------------------------------------------------------
// 0x03 Cauxin — read a single byte from AUX.
// EmuTOS: bdos/console.c:124 `xauxin()`. Routes through Bconin(1).
// NewTOS profile: AUX is the same PL011 as the console; no separate
// device. We return `EUNDEV` until a proper AUX driver lands.
// ---------------------------------------------------------------------------

/// `Cauxin()` — Phase 02 stub: returns `EUNDEV`.
pub fn cauxin(_: &TrapArgs) -> Long {
    EUNDEV
}

// ---------------------------------------------------------------------------
// 0x04 Cauxout — write a single byte to AUX.
// ---------------------------------------------------------------------------

/// `Cauxout(ch)` — Phase 02 stub: returns `EUNDEV`.
pub fn cauxout(_: &TrapArgs) -> Long {
    EUNDEV
}

// ---------------------------------------------------------------------------
// 0x05 Cprnout — write a single byte to the printer.
// EmuTOS: bdos/console.c:148 `xprtout(WORD ch)`.
// NewTOS profile: no printer; spec mandates `EUNDEV` (see
// docs/api/gemdos/console/05-cprnout.md).
// ---------------------------------------------------------------------------

/// `Cprnout(ch)` — printer output; NewTOS has no printer, returns `0`
/// (failure indicator per Compendium for "device not ready").
///
/// Per the EmuTOS source the function returns the number of bytes
/// written; with no printer attached that is always 0.
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
/// (`w == 0x00FF`) returns the byte (low 8 bits) or `0` if no input;
/// the write path returns **`0` always** (not the written byte —
/// EmuTOS `xrawio` falls through to `Bconout(1, w)` and discards
/// the BIOS return).
pub fn crawio(args: &TrapArgs) -> Long {
    let w = args.arg_u16(0);
    let h = host_or_eundev!();
    if w == 0x00FF {
        // Read side: non-blocking; return 0 if no input.
        match h.console_try_read() {
            Some(b) => b as Long,
            None => 0,
        }
    } else {
        h.console_write_byte(w as u8);
        0
    }
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
