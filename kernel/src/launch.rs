//! Kernel-side program launcher — the EmuTOS shell-launch primitive.
//!
//! Loads an external program (registry **or** FAT16 disk) into a process
//! slot, drops to EL0, runs it, and resumes here when it `Pterm`s. This is
//! how the desktop / AUTO folder launch `.PRG` programs — the `NORMAL_APP`
//! / autorun cases of EmuTOS' shell loop.
//!
//! It follows the proven `selftest_pexec` path: `pexec::load` →
//! `enter_el0_with_resume_capture`. Crucially it is **not** a raw `svc`
//! Pexec: the GEMDOS Pexec trap reads its `path` argument as a *user*
//! pointer (`ldtrb`), which faults on a kernel pointer; calling
//! `pexec::load` directly reads the path with `read_file` (an ordinary
//! kernel function), so the caller may pass a kernel string.

use core::sync::atomic::Ordering;

use tos_api::bdos::process::{self, set_current, slot, State, PROCESS_TABLE};

use crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
use crate::kprintln;

/// Load and run the program named `path` (NUL-terminated; a registry key or
/// a FAT16 path like `b"C:\\HELLO.PRG\0"`). Drops to EL0, runs it, and
/// returns when it `Pterm`s. Returns `true` if it launched, `false` on a
/// load/launch failure.
///
/// The desktop / boot caller repaints the screen afterwards (the program
/// owned it while running).
pub fn run_program(path: &[u8]) -> bool {
    // Load from the registry or the FAT16 file system into a fresh slot.
    // SAFETY: single-CPU boot/desktop context; the BDOS is not concurrently
    // locked, and the returned `LoadedChild` borrows nothing past this call.
    let child = match unsafe { process::pexec::load(path, false) } {
        Ok(c) => c,
        Err(e) => {
            kprintln!("[launch] load failed: {:?}", e);
            return false;
        }
    };

    let s = match slot(child.slot) {
        Some(s) => s,
        None => return false,
    };
    s.set_state(State::Running);
    set_current(s.basepage_kva.load(Ordering::Acquire));
    kprintln!("[launch] EL0: slot {} entry 0x{:x}", child.slot, child.entry_va);

    // SAFETY: single-CPU; `parent_frame` captures this kernel resume point,
    // and the program's `Pterm` restores it via the handler's SPSR.M = EL1t
    // (kernel-launched) branch — exactly as `selftest_pexec` does.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            child.entry_va,
            child.l0_pa,
            USER_STACK_TOP_EXCLUSIVE,
            &PROCESS_TABLE[child.slot].parent_frame,
        );
    }

    kprintln!("[launch] returned (program Pterm'd)");
    true
}
