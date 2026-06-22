//! Cooperative round-robin scheduler over the process table.
//!
//! The **run-set** is the set of processes marked [`Process::runnable`]
//! (peers in a voluntary round-robin). A peer gives up the CPU with
//! `Pwait` (GEMDOS 0x4D) or by blocking on input; [`on_yield`] then
//! picks the next runnable peer. On termination, [`on_exit`] drops the
//! peer and picks the next one (or returns `None`, i.e. resume the
//! parent / boot via the existing `Pterm` path).
//!
//! ## Extensible toward preemptive multitasking
//!
//! Everything scheduling-specific lives here: the run-set membership and
//! the `pick_next` policy. The SVC handler only ever sees a [`Switch`]
//! (which process to run and how to enter it). Growing this into a
//! richer scheduler — priorities, time-slices via a timer-driven yield,
//! blocked/sleeping states for `evnt_*` waits — means changing this
//! module and adding fields to [`Process`], **not** touching the trap
//! path. The cooperative `Pwait` hand-off is just the first policy.
//!
//! [`Process`]: super::Process
//! [`Process::runnable`]: super::Process::runnable

use core::sync::atomic::{AtomicUsize, Ordering};

use super::{MAX_PROCESSES, PROCESS_TABLE};

/// Sentinel for "no slot" (the kernel/boot is on the CPU, no peer runs).
const NONE: usize = MAX_PROCESSES;

/// The slot currently on the CPU, or [`NONE`].
static CURRENT: AtomicUsize = AtomicUsize::new(NONE);

/// What the SVC handler must do to switch to the next process. The user
/// stack top and basepage VA are fixed (`el0::USER_STACK_TOP_EXCLUSIVE`,
/// `loader::USER_VA_BASE`) — every process maps them in its own address
/// space — so the handler supplies those constants itself.
pub struct Switch {
    /// Slot to run next.
    pub slot: usize,
    /// `true` = first run (enter at `entry_va` with a fresh stack);
    /// `false` = resume the saved EL0 context.
    pub fresh: bool,
    /// Physical L0 page-table root to install in `TTBR0_EL1`.
    pub l0_pa: u64,
    /// Entry point (user VA), used only when `fresh`.
    pub entry_va: u64,
}

/// The slot currently running (or [`NONE`]).
#[inline]
pub fn current() -> usize {
    CURRENT.load(Ordering::Acquire)
}

/// Record `slot` as the running process.
#[inline]
pub fn set_running(slot: usize) {
    CURRENT.store(slot, Ordering::Release);
}

/// Add `slot` to the cooperative run-set as a fresh (never-entered) peer.
pub fn admit(slot: usize) {
    if slot < MAX_PROCESSES {
        PROCESS_TABLE[slot].has_ctx.store(false, Ordering::Release);
        PROCESS_TABLE[slot].runnable.store(true, Ordering::Release);
    }
}

/// Mark `slot`'s saved context valid (the handler has snapshotted its
/// frame into `saved_frame`); a later switch will *resume* it.
#[inline]
pub fn mark_yielded(slot: usize) {
    if slot < MAX_PROCESSES {
        PROCESS_TABLE[slot].has_ctx.store(true, Ordering::Release);
    }
}

/// Round-robin policy: the next runnable slot strictly after `after`
/// (wrapping), or [`NONE`] if none is runnable. Returns `after` itself
/// when it is the only runnable slot.
fn pick_next(after: usize) -> usize {
    let start = if after >= MAX_PROCESSES { 0 } else { after };
    for k in 1..=MAX_PROCESSES {
        let i = (start + k) % MAX_PROCESSES;
        if PROCESS_TABLE[i].runnable.load(Ordering::Acquire) {
            return i;
        }
    }
    NONE
}

fn switch_to(slot: usize) -> Switch {
    let p = &PROCESS_TABLE[slot];
    Switch {
        slot,
        fresh: !p.has_ctx.load(Ordering::Acquire),
        l0_pa: p.l0_pa.load(Ordering::Acquire),
        entry_va: p.entry_va.load(Ordering::Acquire),
    }
}

/// A peer yielded (`Pwait`). Pick the next runnable peer; `None` means
/// "stay on the current process" (it is the only runnable one).
pub fn on_yield(cur: usize) -> Option<Switch> {
    let next = pick_next(cur);
    if next == NONE || next == cur {
        None
    } else {
        Some(switch_to(next))
    }
}

/// The current peer is terminating: drop it from the run-set and pick the
/// next runnable peer. `None` means no peers remain — the handler should
/// fall back to the ordinary `Pterm` parent/boot resume.
pub fn on_exit(cur: usize) -> Option<Switch> {
    if cur < MAX_PROCESSES {
        PROCESS_TABLE[cur].runnable.store(false, Ordering::Release);
    }
    let next = pick_next(cur);
    if next == NONE || next == cur {
        CURRENT.store(NONE, Ordering::Release);
        None
    } else {
        Some(switch_to(next))
    }
}
