//! GEMDOS memory group (`Maddalt`, `Mxalloc`, `Malloc`, `Mfree`,
//! `Mshrink`).
//!
//! The trap entries in this module decode the AAPCS64 argument
//! registers and forward into [`crate::bdos::mem::traps`]. Argument
//! ordering follows the Atari ABI exactly so source-level ports see
//! the same parameters in the same positions.

use crate::trap::TrapArgs;
use tos_types::Long;

/// `Maddalt(start, size)` — GEMDOS 0x14.
///
/// Spec:
/// [`docs/api/gemdos/memory/14-maddalt.md`](../../../../docs/api/gemdos/memory/14-maddalt.md).
///
/// Argument register layout:
/// - `x0` = `start`  (Atari `LONG`, sign-extended to `i64` on entry)
/// - `x1` = `size`   (Atari `LONG`)
pub fn maddalt(args: &TrapArgs) -> Long {
    let start = args.arg_i32(0);
    let size = args.arg_i32(1);
    // SAFETY: NewTOS is single-CPU pre-Phase 06 and SVC handlers run
    // with IRQs masked, so the BDOS layer is exclusively held for
    // the duration of the dispatch. Phase 06 will replace this with
    // an explicit BDOS giant lock.
    unsafe { crate::bdos::mem::traps::maddalt(start, size) }
}

/// `Mxalloc(amount, mode)` — GEMDOS 0x44.
///
/// Spec:
/// [`docs/api/gemdos/memory/44-mxalloc.md`](../../../../docs/api/gemdos/memory/44-mxalloc.md).
///
/// Argument register layout:
/// - `x0` = `amount` (Atari `LONG`)
/// - `x1` = `mode`   (Atari `WORD`)
pub fn mxalloc(args: &TrapArgs) -> Long {
    let amount = args.arg_i32(0);
    let mode = args.arg_i16(1);
    // SAFETY: see `maddalt`.
    unsafe { crate::bdos::mem::traps::mxalloc(amount, mode) }
}

/// `Malloc(amount)` — GEMDOS 0x48.
///
/// Spec:
/// [`docs/api/gemdos/memory/48-malloc.md`](../../../../docs/api/gemdos/memory/48-malloc.md).
///
/// Argument register layout:
/// - `x0` = `amount` (Atari `LONG`)
pub fn malloc(args: &TrapArgs) -> Long {
    let amount = args.arg_i32(0);
    // SAFETY: see `maddalt`.
    unsafe { crate::bdos::mem::traps::malloc(amount) }
}

/// `Mfree(addr)` — GEMDOS 0x49.
///
/// Spec:
/// [`docs/api/gemdos/memory/49-mfree.md`](../../../../docs/api/gemdos/memory/49-mfree.md).
///
/// Argument register layout:
/// - `x0` = `addr` (Atari `LONG`, the block start address)
pub fn mfree(args: &TrapArgs) -> Long {
    let addr = args.arg_i32(0);
    // SAFETY: see `maddalt`.
    unsafe { crate::bdos::mem::traps::mfree(addr) }
}

/// `Mshrink(n, blk, len)` — GEMDOS 0x4A.
///
/// Spec:
/// [`docs/api/gemdos/memory/4a-mshrink.md`](../../../../docs/api/gemdos/memory/4a-mshrink.md).
///
/// Argument register layout:
/// - `x0` = `n`   (Atari `WORD`, ignored — historic CP/M field)
/// - `x1` = `blk` (Atari `LONG`, block start address)
/// - `x2` = `len` (Atari `LONG`, new block length in bytes)
pub fn mshrink(args: &TrapArgs) -> Long {
    let n = args.arg_i16(0);
    let blk = args.arg_i32(1);
    let len = args.arg_i32(2);
    // SAFETY: see `maddalt`.
    unsafe { crate::bdos::mem::traps::mshrink(n, blk, len) }
}
