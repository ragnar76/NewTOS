//! GEMDOS trap binding (`svc #1`) — the handful of calls the runtime and a
//! small GEM program need.
//!
//! ABI (`docs/sdk/05-abi-pitfalls.md` §A1, `kernel/src/arch/aarch64/
//! exceptions.rs`): function id in `x8` (low 16 bits), arguments in
//! `x0..x7` in their TOS order, the result a sign-extended `LONG` in `x0`;
//! every other register is preserved by the handler. The pattern is the
//! kernel desktop's `kernel/src/desk/dosbind.rs` `dos_trap` (which U6b moves
//! here); unlike that EL1 caller this one runs at EL0, where the exception
//! is taken to `SP_EL1` as well, so `nostack` holds the same way.
//!
//! Function ids: EmuTOS `include/bdosbind.h` / `docs/api/gemdos/`.

use core::ffi::CStr;

/// `Cconws` (GEMDOS 0x09) — write a NUL-terminated string to the console.
const CCONWS: u16 = 0x09;
/// `Malloc` (GEMDOS 0x48).
const MALLOC: u16 = 0x48;
/// `Mfree` (GEMDOS 0x49).
const MFREE: u16 = 0x49;
/// `Pterm` (GEMDOS 0x4C).
const PTERM: u16 = 0x4C;

/// `ENSMEM` (`include/bdosdefs.h`): insufficient memory.
pub const ENSMEM: i32 = -39;
/// `ERROR` (`include/bdosdefs.h`): the generic error.
pub const ERROR: i32 = -1;

/// Issue the GEMDOS trap with up to three arguments.
#[inline]
fn trap1(func: u16, a0: u64, a1: u64, a2: u64) -> i64 {
    let ret: i64;
    // SAFETY: GEMDOS trap. The handler restores every register except `x0`,
    // which it overwrites with the sign-extended `LONG` result. Memory is not
    // declared untouched: calls write through the pointers they are given.
    unsafe {
        core::arch::asm!(
            "svc #1",
            inlateout("x0") a0 => ret,
            in("x1") a1,
            in("x2") a2,
            in("x8") func as u64,
            options(nostack),
        );
    }
    ret
}

/// `Cconws(string)` — write `s` to the console (on NewTOS: the screen and,
/// with `serial=on`, the serial line). Returns the GEMDOS result.
pub fn cconws(s: &CStr) -> i32 {
    trap1(CCONWS, s.as_ptr() as u64, 0, 0) as i32
}

/// `Malloc(amount)` — allocate `amount` bytes of the TPA pool; the block's
/// address, or `None` when GEMDOS answers 0 (no block that large).
///
/// `amount == -1` (the size of the largest free block) is [`malloc_avail`].
pub fn malloc(amount: usize) -> Option<core::ptr::NonNull<u8>> {
    let r = trap1(MALLOC, amount as u64, 0, 0);
    if r <= 0 {
        None
    } else {
        core::ptr::NonNull::new(r as usize as *mut u8)
    }
}

/// `Malloc(-1)` — the size of the largest free block.
pub fn malloc_avail() -> i64 {
    trap1(MALLOC, (-1i64) as u64, 0, 0)
}

/// `Mfree(block)` — return a block obtained from [`malloc`]. `E_OK` (0) or a
/// negative GEMDOS error (`EIMBA`).
pub fn mfree(block: *mut u8) -> i32 {
    trap1(MFREE, block as u64, 0, 0) as i32
}

/// `Pterm(retcode)` — end the program with exit code `rc`. Never returns.
pub fn pterm(rc: i16) -> ! {
    trap1(PTERM, rc as i64 as u64, 0, 0);
    // Pterm does not return; guard against a runaway PC (as sdk/crt0.S does).
    loop {
        core::hint::spin_loop();
    }
}
