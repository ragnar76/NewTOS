//! XBIOS — Trap 14 (`svc #14`).
//!
//! Spec: `docs/api/xbios/_dispatcher.md`. Native AArch64 ABI (as for
//! GEMDOS / BIOS): function id in `x8`, arguments in `x0..`, result in
//! `x0`. The dispatch returns the full 64-bit `x0` value — most XBIOS
//! calls return a `WORD`/`LONG`, but the screen-base calls
//! (`Physbase`/`Logbase`) return a native pointer, so the result is
//! widened to `i64`.
//!
//! Phase 10 implements the screen group; the rest return [`EINVFN`].

pub mod screen;
pub mod system;

use crate::trap::TrapArgs;
use tos_types::error::EINVFN;

// XBIOS function ids.
const PHYSBASE: u16 = 0x02; // screen group (Phase 10)
const LOGBASE: u16 = 0x03;
const GETREZ: u16 = 0x04;
const CURSCONF: u16 = 0x15;
const SETSCREEN: u16 = 0x05;
const SETPALETTE: u16 = 0x06;
const SETCOLOR: u16 = 0x07;
const RANDOM: u16 = 0x11; // system group (Phase 11)
const SETTIME: u16 = 0x16; // clock group — shares the GEMDOS wall clock
const GETTIME: u16 = 0x17;
const KBDVBASE: u16 = 0x22;
const VSYNC: u16 = 0x25;
const BLITMODE: u16 = 0x40;

/// XBIOS Trap-14 dispatcher. Returns the value to load into `x0`.
pub fn dispatch(args: &TrapArgs) -> i64 {
    match args.func_id() {
        PHYSBASE => screen::physbase(),
        LOGBASE => screen::logbase(),
        GETREZ => screen::getrez(),
        SETSCREEN => screen::setscreen(args),
        SETPALETTE => screen::setpalette(args),
        SETCOLOR => screen::setcolor(args),
        CURSCONF => screen::cursconf(args),
        RANDOM => system::random(),
        // Clock group: Settime/Gettime read/write the same wall clock as
        // GEMDOS Tgetdate/Tgettime (no separate RTC on NewTOS). Settime
        // returns void → 0; Gettime returns the packed LONG (zero-extended).
        SETTIME => {
            crate::gemdos::time::xbios_settime(args.arg_i32(0) as u32);
            0
        }
        GETTIME => crate::gemdos::time::xbios_gettime() as i64,
        KBDVBASE => system::kbdvbase(),
        VSYNC => system::vsync(),
        // No Atari BLiTTER on the Pi → contract status word 0x0000
        // (not EINVFN). See `docs/api/xbios/40-blitmode.md`.
        BLITMODE => system::blitmode(),
        _ => EINVFN as i64,
    }
}
