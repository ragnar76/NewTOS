//! BIOS character devices — `Bconstat` / `Bconin` / `Bconout` /
//! `Bcostat` (BIOS 0x01 / 0x02 / 0x03 / 0x08).
//!
//! Spec: `docs/api/bios/{01,02,03,08}-*.md`. EmuTOS dispatches each
//! through a per-handle vector table (`bcon*_vec[8]`). The NewTOS profile
//! has one real character device — the console (PL011 + framebuffer):
//! keyboard input on handles 2 (CON) and 4 (IKBD), screen output on
//! handles 2 (CON) and 5 (raw-CON). The absent devices (0 PRT, 1 AUX,
//! 3 MIDI) follow EmuTOS' `char_dummy` slot semantics — status 0, input
//! 0, output discarded — rather than blocking on hardware that does not
//! exist. All four guard on an installed host (EmuTOS' `CHARDEV_AVAILABLE`
//! boot gate: return 0 before the console is up).

use tos_types::Long;

use crate::host;
use crate::trap::TrapArgs;

// Device handles (EmuTOS `bios/bios.c`).
const CON: u16 = 2;
const IKBD: u16 = 4;
const RAW_CON: u16 = 5;

/// Handles whose input is the console keyboard ring.
fn is_console_in(handle: u16) -> bool {
    handle == CON || handle == IKBD
}

/// Handles whose output is the console screen.
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

/// `Bconin` (0x02) — blocking read from an input device. Console handles
/// return the byte as `0x0000_00NN` (ASCII in the low byte; the scancode
/// high word is 0 — the serial/IKBD source carries ASCII only, the same
/// convention as GEMDOS `Cconin`). Absent devices return `0`.
pub fn bconin(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    match host::host() {
        Some(h) if is_console_in(handle) => h.console_read_byte_blocking() as Long,
        _ => 0,
    }
}

/// `Bconout` (0x03) — write a byte to an output device. Console handles
/// write to the screen and return `1` (byte accepted, per the standard
/// `bconout2`/`bconout5` handlers); absent devices discard and return
/// `0` (`char_dummy`).
pub fn bconout(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    let what = args.arg_u16(1) as u8;
    match host::host() {
        Some(h) if is_console_out(handle) => {
            h.console_write_byte(what);
            1
        }
        _ => 0,
    }
}

/// `Bcostat` (0x08) — output-device status. Console output is always
/// ready (`-1`, the PL011 TX drains by polling); absent devices report
/// busy (`0`).
pub fn bcostat(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    match host::host() {
        Some(_) if is_console_out(handle) => -1,
        _ => 0,
    }
}
