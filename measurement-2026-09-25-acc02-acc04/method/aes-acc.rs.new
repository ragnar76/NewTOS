//! Desk accessories — the AES half of the `.NAC` lifecycle (ACC02-ACC04).
//!
//! Spec: [`docs/api/aes/accessories.md`] (the end-to-end lifecycle extracted
//! from EmuTOS), with [`docs/api/aes/menu.md`] for `mn_register`/`menu_fixup`
//! and [`docs/api/aes/event.md`] for the `AP_MESAG`/`AP_ACCLOSE` flags.
//!
//! This module owns the two things EmuTOS keeps in `aes/geminit.c` that are
//! not menu state: the set of **accessory processes** that exist (EmuTOS finds
//! them by walking `fpdnm(NULL, pid)` over the reserved DA pid range 2…,
//! `geminit.c:616-624`, because its AESPD slots 2..`num_accs+1` *are* the
//! accessory list) and [`wait_for_accs`], the barrier both `run_accs_and_desktop`
//! (`geminit.c:737`) and `ap_exit` (`gemaplib.c:255`) block on.
//!
//! NewTOS has no reserved pid range — an accessory's AES identity is the GEMDOS
//! slot its process got (`aes/pd.rs`, A62) — so the list is explicit here, filled
//! by the kernel-side loader ([`register_process`]). That is a **mechanism**
//! difference only: what a program can observe (which processes the barrier
//! waits for, and that it gives up after `WAIT_TIMEOUT` passes) is EmuTOS's.
//!
//! The loading half (`count_accs`/`load_one_acc`/`load_accs`/`free_accs`) is
//! kernel-side in `kernel/src/accs.rs`: it needs the ELF loader and the
//! process table, which sit below this layer.

use core::cell::UnsafeCell;

use super::pd;
use crate::bdos::process::sched;

/// `AP_MESAG` (`aes/struct.h:116`) — "the application has waited for a
/// message". Re-exported so the kernel-side loader can name the barrier it
/// waits on without reaching into the private descriptor module.
pub const AP_MESAG: u16 = pd::AP_MESAG;
/// `AP_ACCLOSE` (`aes/struct.h:117`) — "the application has seen an AC_CLOSE
/// message". Re-exported alongside [`AP_MESAG`].
pub const AP_ACCLOSE: u16 = pd::AP_ACCLOSE;

/// `NUM_ACCS` (`include/sysconf.h:37-40`) — "the maximum number of desk
/// accessory files that will be loaded **AND** the maximum number of desk
/// accessory slots available (one slot per `mn_register()` call)": one shared
/// budget, not two. `sysconf.h`'s own header states these definitions are not
/// overridable, so this is unconditional in every EmuTOS build.
pub const NUM_ACCS: usize = 6;

/// `WAIT_TIMEOUT` (`aes/geminit.c:102`) — "as a precaution against infinite
/// loops, we set a count timeout. in limited testing, the maximum number of
/// loops actually used was less than 100." [`wait_for_accs`] gives up after
/// this many passes and returns as if the flag had been set.
pub const WAIT_TIMEOUT: u32 = 500;

/// The AES identities (process-descriptor indices) of the loaded accessory
/// processes, in load order — EmuTOS's AESPD slots 2..`num_accs+1`.
/// `usize::MAX` is the empty entry.
struct AccPds(UnsafeCell<[usize; NUM_ACCS]>);
// SAFETY: single boot CPU; touched only from the AES/boot path.
unsafe impl Sync for AccPds {}
static ACC_PDS: AccPds = AccPds(UnsafeCell::new([usize::MAX; NUM_ACCS]));

/// Borrow the accessory identity table.
///
/// # Safety
/// Single AES/boot path (no re-entrancy).
#[allow(clippy::mut_from_ref)]
unsafe fn table() -> &'static mut [usize; NUM_ACCS] {
    // SAFETY: see `AccPds`'s Sync impl.
    unsafe { &mut *ACC_PDS.0.get() }
}

/// Record that identity `idx` is a desk-accessory process — the loader's
/// counterpart of EmuTOS reserving AESPD slot `2 + n` for the `n`-th `.ACC`
/// (`geminit.c:819,851-877`). Ignores a repeat registration and anything past
/// the [`NUM_ACCS`] budget.
///
/// Also puts the slot into the cooperative run-set ([`sched::admit`]) as a
/// **fresh** peer: like EmuTOS's `pstart(gotopgm, …)` (`geminit.c:255`) this
/// only makes the process schedulable — it does not run until something
/// dispatches, which for us is the first [`wait_for_accs`] / blocking
/// `evnt_*` (EmuTOS's `dsptch()`, `geminit.c:736`).
pub fn register_process(idx: usize) {
    // SAFETY: single AES/boot path.
    let t = unsafe { table() };
    if t.contains(&idx) {
        return;
    }
    if let Some(free) = t.iter_mut().find(|e| **e == usize::MAX) {
        *free = idx;
        sched::admit(idx);
    }
}

/// Forget accessory process `idx` — used when a loaded accessory terminates
/// (EmuTOS's never do, `docs/api/aes/accessories.md` §4 point 6) so the
/// barriers below stop waiting for a process that is gone.
pub fn forget_process(idx: usize) {
    // SAFETY: single AES/boot path.
    for e in unsafe { table() }.iter_mut() {
        if *e == idx {
            *e = usize::MAX;
        }
    }
}

/// The identities of the loaded accessory processes, in load order.
pub fn processes() -> impl Iterator<Item = usize> {
    // SAFETY: single AES/boot path; the returned values are plain indices.
    let t = *unsafe { table() };
    t.into_iter().filter(|e| *e != usize::MAX)
}

/// Number of loaded accessory processes.
pub fn count() -> usize {
    processes().count()
}

/// Drop the whole accessory list — the AES-restart half (`mn_init`'s
/// neighbourhood, [`super::reinit`]). The processes themselves are freed by
/// the kernel's `free_accs`.
pub(super) fn reinit() {
    // SAFETY: single restart path, no AES call in flight.
    unsafe { table() }.fill(usize::MAX);
}

/// `true` when every loaded accessory has `bitmask` set in its `p_flags` —
/// `wait_for_accs`'s inner loop (`geminit.c:616-624`).
pub fn all_have(bitmask: u16) -> bool {
    processes().all(|idx| {
        // SAFETY: single AES path; `by_index` hands out the live descriptor.
        match unsafe { pd::by_index(idx) } {
            Some(p) => p.flags & bitmask != 0,
            None => true,
        }
    })
}

/// `wait_for_accs(bitmask)` (`aes/geminit.c:599-628`) — block until every desk
/// accessory has `bitmask` set in its `p_flags`, or [`WAIT_TIMEOUT`] passes
/// have gone by.
///
/// EmuTOS's body is `all_run(); check every DA's p_flags;` in a bounded loop:
/// `all_run()` puts every accessory back in the ready list and the check runs
/// after the dispatcher has let them run. Here the two halves are the
/// scheduler's: [`sched::sleep`] hands the CPU to a runnable accessory
/// (returning [`sched::SleepOutcome::Yield`], which the caller must propagate
/// to the trap handler as the S2 restartable block) or, with none runnable,
/// runs the idle pass and comes back for another look.
///
/// Returns `Some(switch)` when the caller must abort its syscall and yield —
/// the wait resumes from the top when the caller's `svc` re-executes — and
/// `None` when the barrier is satisfied (or timed out, as EmuTOS's is).
///
/// **Restart-safety.** The function commits nothing: it only reads `p_flags`.
/// The `TIMEOUT_PASSES` counter is deliberately *not* carried across a yield —
/// EmuTOS's `n` is not either, in the sense that its loop never leaves the
/// function; a caller that yields and re-enters simply gets a fresh budget,
/// which can only make the barrier more patient, never less.
#[must_use]
pub fn wait_for_accs(bitmask: u16) -> Option<sched::Switch> {
    for _ in 0..WAIT_TIMEOUT {
        if all_have(bitmask) {
            return None;
        }
        match sched::sleep(sched::wait_input()) {
            sched::SleepOutcome::Yield(sw) => return Some(sw),
            sched::SleepOutcome::Recheck => {}
        }
    }
    // `KDEBUG(("wait_for_accs(): %8.8s took too long\n", pd->p_name))` (`:627`)
    // — EmuTOS gives up and carries on, and so do we.
    None
}
