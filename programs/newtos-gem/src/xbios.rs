//! XBIOS trap binding (`svc #14`) — EmuTOS's `include/xbiosbind.h`.
//!
//! DESK43 U6b: moved here from the kernel desktop's
//! `kernel/src/desk/xbiosbind.rs`. The desktop takes exactly one function from
//! `xbiosbind.h`: `Keytbl(-1, -1, -1)`, the Alt-letter drive shortcut's source
//! for the shifted character of a scancode (`desk/deskmain.c:1136-1159`).
//!
//! ## ABI
//!
//! `docs/ARCHITECTURE.md`, "Native AArch64 callers": function ID in `x8` (low
//! 16 bits), arguments in `x0..x7` in their TOS order, result in `x0` — the
//! shape of [`crate::gemdos`]'s trap, with the SVC immediate 14 instead of 1
//! (`tos-api/src/trap.rs:40-48`).
//!
//! **The result is kept 64 bits wide here, unlike the GEMDOS `LONG`.**
//! `tos_types::Long` is `i32` — the m68k LONG, which on m68k is also the
//! pointer width. It is not ours: `Keytbl` returns an *address*
//! (`bios/ikbd.c:133`), and on AArch64 that is a 64-bit value the XBIOS
//! dispatcher writes into `x0` whole (`tos-api/src/xbios/keytbl.rs`, returning
//! `emit_base as i64`). Narrowing it truncates a kernel caller's high-half
//! prefix and yields a low-half address that faults on the first dereference —
//! measured, not assumed (`measurements/measurement-2026-09-26-desk43-u4/serial/`,
//! `[EXC] … Data Abort … FAR=0x00000000002fcaa8` for a `KERNEL_AREA` whose real
//! VA is `0xffff0000002fcaa8`).

/// XBIOS `Keytbl` — `#define Keytbl(a,b,c) xbios_l_lll(16,a,b,c)`
/// (`include/xbiosbind.h:34`).
// NEWTOS: EmuTOS names no constant for the number; the macro is its name.
pub const KEYTBL: u16 = 16;

/// `(UBYTE *)-1` — `Keytbl`'s "keep the current table" sentinel
/// (`bios/ikbd.c:124-132`).
pub const KEYTBL_KEEP: u64 = u64::MAX;

/// Issue the XBIOS trap (`svc #14`): function id in `x8`, arguments in
/// `x0..x2`. EmuTOS's `trap14()` (`util/miscasm.S`), reached through
/// `xbiosbind.h`'s `xbios_l_lll`.
///
/// `x3..x6` are zeroed for the reason the GEMDOS trap zeroes them: the
/// dispatcher's tracer records the whole frame, and leftovers in the unused
/// argument registers would be noise in it. No XBIOS function bound here takes
/// more than three arguments.
///
/// `nostack`: an exception taken at EL1 with `SPSel = 0` (the kernel desktop)
/// or at EL0 runs the handler on `SP_EL1`, so the caller's stack is untouched.
/// Memory is deliberately *not* declared clean — `Keytbl` rewrites the
/// `current_keytbl` image the caller is about to read.
#[inline]
fn trap14(func: u16, a0: u64, a1: u64, a2: u64) -> u64 {
    let ret: u64;
    // SAFETY: XBIOS trap. The handler restores every register except `x0`,
    // which it overwrites with the dispatcher's result.
    unsafe {
        core::arch::asm!(
            "svc #14",
            inlateout("x0") a0 => ret,
            in("x1") a1,
            in("x2") a2,
            in("x3") 0u64,
            in("x4") 0u64,
            in("x5") 0u64,
            in("x6") 0u64,
            in("x8") func as u64,
            options(nostack),
        );
    }
    ret
}

/// `Keytbl(unshift, shift, capslock)` (`xbiosbind.h:34`, `bios/ikbd.c:122-134`)
/// — install the given tables ([`KEYTBL_KEEP`] keeps one) and return the
/// address of the `struct keytbl` describing the layout now in force.
///
/// **0** means the board has no keyboard translator to describe; the contract
/// there is "do not dereference" (`docs/api/xbios/10-keytbl.md`). For an EL1
/// caller (no EL0 process current) the XBIOS materialises the image in a
/// kernel static and returns its kernel VA (`tos-api/src/xbios/keytbl.rs`,
/// "Where `current_keytbl` lives").
pub fn keytbl(unshift: u64, shift: u64, capslock: u64) -> u64 {
    trap14(KEYTBL, unshift, shift, capslock)
}
