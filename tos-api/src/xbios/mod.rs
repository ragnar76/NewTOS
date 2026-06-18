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

use crate::trap::TrapArgs;
use tos_types::error::EINVFN;

// XBIOS function ids (Phase 10 screen group).
const PHYSBASE: u16 = 0x02;
const LOGBASE: u16 = 0x03;
const GETREZ: u16 = 0x04;
const SETSCREEN: u16 = 0x05;
const SETPALETTE: u16 = 0x06;
const SETCOLOR: u16 = 0x07;

/// XBIOS Trap-14 dispatcher. Returns the value to load into `x0`.
pub fn dispatch(args: &TrapArgs) -> i64 {
    match args.func_id() {
        PHYSBASE => screen::physbase(),
        LOGBASE => screen::logbase(),
        GETREZ => screen::getrez(),
        SETSCREEN => screen::setscreen(args),
        SETPALETTE => screen::setpalette(args),
        SETCOLOR => screen::setcolor(args),
        _ => EINVFN as i64,
    }
}
