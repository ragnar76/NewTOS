//! `desk::xbiosbind` — the desktop's **XBIOS trap binding**, EmuTOS's
//! `include/xbiosbind.h`.
//!
//! The third of the desktop's system interfaces, next to [`super`]'s aesbind
//! (`svc #2`, selector 200, `desk/gembind.c`), [`super::dosbind`] (`svc #1`,
//! `include/gemdos.h`) and [`super::vdibind`] (`svc #2`, selector 115). A
//! desktop that is a client of the operating system reaches an XBIOS function
//! the way an application reaches it — through `trap #14` — and not by naming
//! a kernel symbol.
//!
//! ## What the desktop takes from `xbiosbind.h`
//!
//! Exactly one function: `Keytbl(-1, -1, -1)`, the Alt-letter drive shortcut's
//! source for the shifted character of a scancode
//! (`desk/deskmain.c:1136-1159`). EmuTOS's desktop also uses `Supexec`
//! (`desksupp.c:219`) and `deskrez.c`'s resolution change; NewTOS has neither
//! (the one piece of state the `Supexec` fetches is the `_IDT` cookie, for
//! which [`super::dosbind`]'s `dos_getcookie` is the route, and the screen mode
//! is fixed by the firmware).
//!
//! ## ABI
//!
//! `docs/ARCHITECTURE.md`, "Native AArch64 callers": function ID in `x8` (low
//! 16 bits), arguments in `x0..x7` in their TOS order, result in `x0` — the
//! same shape [`super::dosbind`]'s `dos_trap` uses, with the SVC immediate 14
//! instead of 1 (`tos-api/src/trap.rs:40-48`).
//!
//! **The result is kept 64 bits wide here, unlike `dos_trap`'s `Long`.**
//! `tos_types::Long` is `i32` — the m68k LONG, which on m68k is also the
//! pointer width. It is not ours: `Keytbl` returns an *address*
//! (`bios/ikbd.c:133`), and on AArch64 that is a 64-bit value the XBIOS
//! dispatcher writes into `x0` whole (`tos-api/src/xbios/keytbl.rs`, returning
//! `emit_base as i64`). Narrowing it to `Long` truncates the kernel's high-half
//! prefix and yields a low-half address that faults on the first dereference —
//! measured, not assumed (`measurement-2026-09-26-desk43-u4/serial/`,
//! `[EXC] … Data Abort … FAR=0x00000000002fcaa8` for a `KERNEL_AREA` whose real
//! VA is `0xffff0000002fcaa8`).

/// XBIOS `Keytbl` — `#define Keytbl(a,b,c) xbios_l_lll(16,a,b,c)`
/// (`include/xbiosbind.h:34`).
const KEYTBL: u16 = 16;

/// `(UBYTE *)-1` — `Keytbl`'s "keep the current table" sentinel
/// (`bios/ikbd.c:124-132`).
const KEEP: u64 = u64::MAX;

/// Issue the XBIOS trap (`svc #14`): function id in `x8`, arguments in
/// `x0..x2`. EmuTOS's `trap14()` (`util/miscasm.S`), reached through
/// `xbiosbind.h`'s `xbios_l_lll`.
///
/// `x3..x6` are zeroed for the reason [`super::dosbind::dos_trap`] zeroes
/// them: the dispatcher's tracer records the whole frame, and leftovers in the
/// unused argument registers would be noise in it. No XBIOS function the
/// desktop calls takes more than three arguments.
///
/// `nostack`: an exception taken at EL1 with `SPSel = 0` runs the handler on
/// `SP_EL1` (the emergency stack, `link-rpi4.ld:100-102`), so the caller's
/// stack is untouched. Memory is deliberately *not* declared clean — `Keytbl`
/// rewrites the `current_keytbl` image the caller is about to read.
#[inline]
fn xbios_trap(func: u16, a0: u64, a1: u64, a2: u64) -> u64 {
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

/// `Keytbl(-1, -1, -1)` (`xbiosbind.h:34`, `bios/ikbd.c:122-134`) — install
/// nothing and return the address of the `struct keytbl` describing the
/// keyboard layout now in force.
///
/// The three sentinel arguments are the only form the desktop uses
/// (`deskmain.c:1144`): it reads the tables, it never installs one.
///
/// The returned address is dereferenceable by this caller: with no EL0 process
/// current — the desktop's kernel-boot context — the XBIOS materialises the
/// image in a kernel static and returns its kernel VA
/// (`tos-api/src/xbios/keytbl.rs`, "Where `current_keytbl` lives"). **0** means
/// the board has no keyboard translator to describe; the contract there is
/// "do not dereference", and [`super::main::check_alt_letter_key`] honours it.
pub(super) fn keytbl_query() -> u64 {
    xbios_trap(KEYTBL, KEEP, KEEP, KEEP)
}
