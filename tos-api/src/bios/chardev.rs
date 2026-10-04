//! BIOS character devices — `Bconstat` / `Bconin` / `Bconout` /
//! `Bcostat` (BIOS 0x01 / 0x02 / 0x03 / 0x08).
//!
//! Spec: `docs/api/bios/{01,02,03,08}-*.md`. EmuTOS dispatches each
//! through a per-handle vector table (`bcon*_vec[8]`). The NewTOS profile
//! has one real character device — the console (PL011 + framebuffer):
//! keyboard **input** on handle 2 (CON) only, screen **output** on
//! handles 2 (CON) and 5 (raw-CON). The absent devices (0 PRT, 1 AUX,
//! 3 MIDI, 4 IKBD-TX) follow EmuTOS' `char_dummy` slot semantics —
//! status 0, input 0, output discarded — rather than blocking on hardware
//! that does not exist. All four guard on an installed host (EmuTOS'
//! `CHARDEV_AVAILABLE` boot gate: return 0 before the console is up).

use tos_types::Long;

use crate::host;
use crate::trap::TrapArgs;

// Device handles (EmuTOS `bios/bios.c`, `bios/chardev.c`).
const CON: u16 = 2;
const RAW_CON: u16 = 5;

/// Handles whose input is the console keyboard ring. **Only CON (handle
/// 2).** EmuTOS routes keyboard input through `bconin2` / `bconstat2`
/// (the IKBD queue); handle 4 (IKBD) is an *output* channel
/// (keyboard-controller TX) whose *input* slot is `char_dummy`
/// (`bios/chardev.c:34-36`, `bconin_init[4] = char_dummy`), so
/// `Bconstat(4)` and `Bconin(4)` return `0` immediately. Treating 4 as a
/// console-input handle made `Bconin(4)` block forever waiting for a key
/// that never routes there (the handle-4 hang).
fn is_console_in(handle: u16) -> bool {
    handle == CON
}

/// Handles whose output is the console screen: CON (`bconout2`) and
/// raw-CON (`bconout5` → `ascii_out`), both `bios/chardev.c:43-45`. Note
/// this is *not* symmetric with output *status*: `Bcostat` reports ready
/// only for CON — handle 5's `bcostat` slot is `char_dummy` (see
/// [`bcostat`]).
fn is_console_out(handle: u16) -> bool {
    handle == CON || handle == RAW_CON
}

/// `Bconstat` (0x01) — input-device status. `-1` when a byte is ready on
/// a console-input handle, else `0` (also the boot-guard / absent-device
/// value).
pub fn bconstat(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    match host::host() {
        Some(h) if is_console_in(handle) && h.console_has_input() => -1,
        _ => 0,
    }
}

/// Per-process `conterm` sysvar address ($484) — the classic TOS
/// console-attribute byte the loader's `populate_sysvars` writes into every
/// TPA (default 7, EmuTOS `bios/ikbd.c:1100`).
const CONTERM_ADDR: usize = 0x484;
/// `conterm` bit 3: fold the keyboard shift state into `Bconin`'s return
/// LONG (EmuTOS `bios/ikbd.c:279`).
const CONTERM_SHIFT_FOLD: u8 = 0x08;

/// `conterm` bit 1: keyboard auto-repeat enabled (EmuTOS `bios/ikbd.c:475`).
pub const CONTERM_KEY_REPEAT: u8 = 0x02;

/// The `conterm` byte (`$484`) in effect for the keyboard right now: the
/// running process's own copy, or the boot value `7` (EmuTOS `kbd_init`,
/// `bios/ikbd.c:1100`) when no process runs (boot, the built-in desktop).
///
/// Read through the **kernel high-half alias** of the TPA that `RUN` holds,
/// like `bios::tick`'s `_hz_200` publish — not through the user mapping — so
/// it is safe from any EL1 context regardless of which `TTBR0` is installed.
/// Used by the key-repeat handler (`kernel::ikbd::poll_repeat`, conterm bit 1
/// = [`CONTERM_KEY_REPEAT`]).
pub fn current_conterm() -> u8 {
    let bp = crate::bdos::process::current();
    if bp.is_null() {
        return crate::bdos::process::loader::CONTERM_DEFAULT;
    }
    // SAFETY: `RUN` is the kernel VA of a live process's TPA base (published
    // by the loader, cleared on `Pterm` inside the IRQ-masked SVC handler);
    // `$484` lies in its first page, in the loader-populated sysvar area
    // (`populate_sysvars`). Same reasoning as `bios::tick::publish`.
    unsafe { (bp as *const u8).add(CONTERM_ADDR).read_volatile() }
}

/// Pack an IKBD key word (`scancode << 8 | ascii`) into the console
/// `Bconin`/`Cconin` return LONG: bits 0-7 = ASCII (0 for non-ASCII keys),
/// bits 8-15 = 0, bits 16-23 = IKBD scancode. Bits 24-31 carry the keyboard
/// shift state **iff** the calling process's `conterm` has bit 3 set
/// (`conterm & 8`), else they are 0.
///
/// This mirrors EmuTOS `bconin2` (`references/emutos/bios/bios.c:948-956`
/// documents the return format; `bios/ikbd.c:279-280` is the fold), which
/// keeps the IOREC's stored shift byte (bits 24-31, from `kb_last.k.shifty`,
/// `bios/ikbd.c:900`) only when `conterm & 8` and otherwise masks it out
/// (`value &= 0x00ffffff`). The default `conterm = 7` (bit 3 clear) leaves
/// bits 24-31 at 0, byte-identical to a no-fold pack. Every NewTOS caller of
/// this function is a CON-input path (`Bconin(2)`, `Cconin`/`Cnecin`/
/// `Crawcin`/`Crawio` — all of which route through `bconin2` under EmuTOS),
/// so folding here matches EmuTOS for each. See `docs/api/bios/02-bconin.md`,
/// `docs/api/bios/0b-kbshift.md`, `docs/api/gemdos/console/01-cconin.md`.
pub fn pack_console_key(word: u16) -> Long {
    let ascii = (word & 0xFF) as Long;
    let scancode = (word >> 8) as Long;
    (scancode << 16) | ascii | shift_fold()
}

/// The `conterm & 8` keyboard-shift-state contribution to bits 24-31 of a
/// console `Bconin`/`Cconin` return LONG (EmuTOS `bconin2`,
/// `bios/ikbd.c:279-280`). Returns `(kbshift & 0xFF) << 24` when the caller's
/// `conterm` bit 3 is set, else `0`.
///
/// Applied only while an EL0 application is the running process
/// ([`crate::bdos::process::current`] non-null): the app's `TTBR0` is active
/// in this BIOS/GEMDOS trap handler, so `$484` (written by the loader's
/// `populate_sysvars`) is mapped and the unprivileged load reaches it. With
/// **no active user context** — boot, the desktop, or an EL1 selftest, all of
/// which run with `RUN == null` and no per-process low-memory area — the fold
/// is skipped rather than faulting an unprivileged load into the empty
/// `TTBR0` (`EMPTY_L0`). Gating on `RUN` (a stable atomic) rather than
/// `SPSR_EL1` keeps the check correct across `console_read_key_blocking`'s
/// IRQ/`wfi` wait (qemu-virt), which clobbers the saved-PSTATE register but
/// never touches `RUN` or `TTBR0`.
#[inline]
fn shift_fold() -> Long {
    // No active user context (RUN == null) → no per-process $484; skip
    // without touching low memory, so an EL1 caller (desktop / boot / a
    // selftest) never faults an unprivileged load into an empty TTBR0.
    if crate::bdos::process::current().is_null() {
        return 0;
    }
    // App context: read conterm via an unprivileged byte load, exactly like
    // `setexc`'s low-memory peek (`bios/vectors.rs`) / `Cconws`'s `ldtrb`.
    // SAFETY: $484 is inside the per-process reserved low region the loader
    // maps + populates; the app's TTBR0 is active in this trap and the load
    // is permission-checked as EL0.
    let conterm = unsafe { read_user_byte(CONTERM_ADDR as *const u8) };
    if conterm & CONTERM_SHIFT_FOLD == 0 {
        return 0; // classic default (conterm = 7): bits 24-31 = 0
    }
    // conterm & 8: fold NewTOS's `Kbshift` (BIOS fn 11) state into bits
    // 24-31, matching EmuTOS's stored `shifty` byte (`bios/ikbd.c:900,279`).
    let shift = host::host().map_or(0u8, |h| h.keyboard_shift_state());
    ((shift as u32) << 24) as Long
}

/// Read one byte from a user (EL0) pointer via an unprivileged load
/// (`ldtrb`), so the access is permission-checked as EL0 — the same contract
/// as `bios::vectors`'s `read_user_long` and `Cconws`'s `ldtrb`.
///
/// # Safety
///
/// `p` must address a readable EL0 byte under the active `TTBR0`.
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

/// `Bconin` core keyed by device handle — the value part of `Bconin`
/// without the [`TrapArgs`] marshalling. For the CON handle (2) it blocks
/// and returns the packed console LONG (IKBD scancode in bits 16-23, ASCII
/// in bits 0-7); every absent device returns `0` immediately (`char_dummy`).
///
/// Shared with the GEMDOS `Cauxin` pass-through, which routes through
/// `Bconin(1)` exactly as EmuTOS' `xauxin` does (`bdos/console.c:432`);
/// handle 1 (AUX) is an absent device in the NewTOS profile, so it yields
/// `0` rather than blocking on serial hardware that does not exist.
pub(crate) fn bconin_handle(handle: u16) -> Long {
    match host::host() {
        Some(h) if is_console_in(handle) => match h.console_read_key_restartable() {
            Some(word) => pack_console_key(word),
            None => {
                // A `kbdvec` hook has bytes waiting: restart after it ran
                // (KEYS02); the result is discarded by the trap handler.
                crate::xbios::kbdvecs::request_restart();
                0
            }
        },
        _ => 0,
    }
}

/// `Bconin` (0x02) — blocking read from an input device. For the CON
/// handle (2) the packed LONG carries the IKBD scancode in bits 16-23 and
/// the ASCII value in bits 0-7 (non-ASCII keys → ASCII 0 + scancode), per
/// `docs/api/bios/02-bconin.md`. Absent devices return `0`.
pub fn bconin(args: &TrapArgs) -> Long {
    bconin_handle(args.arg_u16(0))
}

/// `Bconout` core keyed by device handle — the value part of `Bconout`
/// without the [`TrapArgs`] marshalling. Console handles (CON, raw-CON)
/// write to the screen and return `1`; every absent device discards the
/// byte and returns `0` (`char_dummy`).
///
/// Shared with the GEMDOS `Cauxout` pass-through, which routes through
/// `Bconout(1, ch)` exactly as EmuTOS' `xauxout` does
/// (`bdos/console.c:331`); handle 1 (AUX) is an absent device in the
/// NewTOS profile, so it discards and returns `0`.
pub(crate) fn bconout_handle(handle: u16, byte: u8) -> Long {
    match host::host() {
        Some(h) if is_console_out(handle) => {
            h.console_write_byte(byte);
            1
        }
        _ => 0,
    }
}

/// `Bconout` (0x03) — write a byte to an output device. Console handles
/// write to the screen and return `1` (byte accepted, per the standard
/// `bconout2`/`bconout5` handlers); absent devices discard and return
/// `0` (`char_dummy`).
pub fn bconout(args: &TrapArgs) -> Long {
    bconout_handle(args.arg_u16(0), args.arg_u16(1) as u8)
}

/// `Bcostat` (0x08) — output-device status. Ready (`-1`) only for CON
/// (handle 2 → `bcostat2`, `bios/chardev.c:112-118`): the PL011 TX drains
/// by polling so it is always ready. Every other handle — including
/// raw-CON (handle 5), whose `bcostat` slot is `char_dummy`
/// (`bcostat_init[5]`, `bios/chardev.c:38-41`) — reports busy (`0`), as do
/// the boot guard and absent devices. This is deliberately **not** the
/// same handle set as `Bconout`: `Bconout(5)` writes and returns `1`, but
/// `Bcostat(5)` is `0`.
pub fn bcostat(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    match host::host() {
        Some(_) if handle == CON => -1,
        _ => 0,
    }
}
