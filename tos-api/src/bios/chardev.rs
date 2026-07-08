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

/// Pack an IKBD key word (`scancode << 8 | ascii`) into the console
/// `Bconin`/`Cconin` return LONG: bits 0-7 = ASCII (0 for non-ASCII keys),
/// bits 8-15 = 0, bits 16-23 = IKBD scancode, bits 24-31 = 0. Shift state
/// is *not* folded in — that is a `Kbshift`/`conterm & 8` concern, not this
/// call's (`docs/api/bios/02-bconin.md`, `docs/api/gemdos/console/01-cconin.md`).
pub fn pack_console_key(word: u16) -> Long {
    let ascii = (word & 0xFF) as Long;
    let scancode = (word >> 8) as Long;
    (scancode << 16) | ascii
}

/// `Bconin` (0x02) — blocking read from an input device. For the CON
/// handle (2) the packed LONG carries the IKBD scancode in bits 16-23 and
/// the ASCII value in bits 0-7 (non-ASCII keys → ASCII 0 + scancode), per
/// `docs/api/bios/02-bconin.md`. Absent devices return `0`.
pub fn bconin(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    match host::host() {
        Some(h) if is_console_in(handle) => pack_console_key(h.console_read_key_blocking()),
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
