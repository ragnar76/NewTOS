//! Run the `lineecho` program — the first *real compiled* NewTOS user
//! program (built from `programs/lineecho`, not a hand-assembled byte
//! array). It prompts, reads a line with `Cconrs`, echoes it back, and
//! exits via `Pterm`. This simultaneously is the end-to-end test of the
//! `Cconrs` line editor from EL0.
//!
//! Because `Cconrs` blocks until a full line arrives, the launch is
//! gated on `Cconis` (input ready) with a short budget so a no-input
//! boot skips it instead of hanging. Pipe a line (ending in CR) to the
//! serial console to see it run — e.g.:
//!
//! ```sh
//! ( while :; do printf 'hi\r'; sleep 0.1; done ) | cargo xtask run
//! ```

use core::sync::atomic::Ordering;

use tos_api::bdos::process::loader::load_to_slot;
use tos_api::bdos::process::{set_current, slot, State, PROCESS_TABLE};

use crate::arch::aarch64::el0::USER_STACK_TOP_EXCLUSIVE;
use crate::console::CONSOLE;
use crate::generated::lineecho_elf::LINEECHO_ELF;
use crate::hal::uart::Uart;
use crate::kprintln;

/// Process slot for the lineecho demo (0–4 are touched by earlier
/// tests; all freed by now, but a dedicated slot keeps it isolated).
const LINEECHO_SLOT: usize = 4;

/// `Cconis()` (GEMDOS 0x0B) — non-zero if console input is ready.
#[inline(always)]
fn cconis() -> i32 {
    let r: i64;
    // SAFETY: SVC #1, constant function id, no args.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x000Bu64, out("x0") r, options(nostack));
    }
    r as i32
}

/// Launch `lineecho` if console input is waiting; otherwise skip.
pub fn run_lineecho_demo() {
    kprintln!("");
    kprintln!("[lineecho] === real compiled program demo ===");

    // Gate on input so the blocking Cconrs inside the program does not
    // hang a no-input boot. ~2 s budget.
    let mut budget: u32 = 20_000_000;
    let ready = loop {
        if cconis() != 0 {
            break true;
        }
        budget = budget.saturating_sub(1);
        if budget == 0 {
            break false;
        }
    };
    if !ready {
        kprintln!("[lineecho] no input (timeout) — skipped (pipe a 'line\\r' to run it)");
        return;
    }

    // Load the real ELF into a slot (same path Pexec uses).
    // SAFETY: pre-test, single-CPU; slot dedicated to this demo.
    let loaded = match unsafe { load_to_slot(&LINEECHO_ELF, LINEECHO_SLOT) } {
        Ok(img) => img,
        Err(e) => {
            kprintln!("[lineecho] === FAIL: loader rejected program: {:?} ===", e);
            return;
        }
    };
    kprintln!(
        "[lineecho] loaded slot {}: entry_va=0x{:x} ({} byte ELF)",
        loaded.slot, loaded.entry_va, LINEECHO_ELF.len(),
    );

    let proc_slot = slot(LINEECHO_SLOT).expect("slot in range");
    proc_slot.set_state(State::Running);
    set_current(proc_slot.basepage_kva.load(Ordering::Acquire));

    // Drop to EL0. The program runs Cconws → Cconrs → Cconws → Pterm;
    // its Pterm returns control here via the kernel-resume path.
    // SAFETY: pre-test, single-CPU; the slot's parent_frame is the
    // kernel-resume capture consumed by the program's Pterm.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            loaded.entry_va,
            loaded.l0_pa,
            USER_STACK_TOP_EXCLUSIVE,
            &PROCESS_TABLE[loaded.slot].parent_frame,
        );
    }

    let ok = proc_slot.read_state() == State::Free
        && tos_api::bdos::process::current().is_null();
    if ok {
        CONSOLE.write_str("[lineecho] === program ran + exited cleanly: OK ===\n");
    } else {
        CONSOLE.write_str("[lineecho] === FAIL: did not clean up ===\n");
    }
}
