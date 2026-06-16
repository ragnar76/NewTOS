//! `Pterm` (GEMDOS 0x4C) and `Pterm0` (0x00) — process termination.
//!
//! Spec: [`docs/api/gemdos/process/01-pterm.md`].
//!
//! ## Phase 06 D6 scope
//!
//! D6 wires the *cleanup* and *parent-resume* halves of `Pterm`.
//! Specifically:
//!
//!   * Walk the dying process's BDOS allocations via
//!     [`crate::bdos::mem::free_all_owned::free_all_owned`] and
//!     return them to both pools' free lists.
//!   * Read the dying slot's `parent_resume_pc` / `parent_kernel_sp`
//!     atomics (captured at the time the loader / `Pexec` dropped to
//!     EL0) so the kernel SVC handler can rewrite the trap frame
//!     and `eret` to the parent's saved EL1 PC.
//!   * Free the slot via [`super::free_slot`] and clear `RUN`.
//!
//! What D6 does **not** yet do:
//!
//!   * The user-term handler at exception number `0x102` (Compendium
//!     `Setexc(0x102, -1)`). Phase 09 introduces the per-process
//!     exception handler table.
//!   * Standard / non-standard file-handle cleanup (`p_uft[]`,
//!     `sft[]`). Phase 08 lands the file-system stack.
//!   * `dirtbl[]` decrement. Same Phase-08 dependency.
//!
//! Until those phases land the cleanup is "memory-only" — but for
//! the D6 selftest (single user `Mxalloc` followed by `Pterm`) that
//! is exactly the right behaviour, and it is also what every
//! source-port program with no open files / no `Dsetdir` calls does
//! at exit.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::Ordering;

use tos_types::BasePage;

use crate::bdos::mem::free_all_owned::free_all_owned;

use super::{free_slot, set_current, slot_for, RUN};

/// Trap-frame redirect context the SVC handler installs to land
/// EL1 (kernel-resume) or EL0 (real Pexec parent) at the parent's
/// saved frame.
///
/// The handler does the actual frame copy in kernel space (it has
/// `&mut ExceptionFrame` and we don't expose that across crates);
/// this struct hands back the parent's slot reference + the
/// metadata needed for the post-cleanup TTBR0 restore.
#[derive(Copy, Clone, Debug)]
pub struct PtermResume {
    /// Phys of the parent's `L0` page table the handler should
    /// install in `TTBR0_EL1` after the cleanup. `0` means "no
    /// parent process — restore the Stage-1 boot tables instead",
    /// which is the Phase-06-D6 path (boot launches the test
    /// process directly without a `Pexec` chain).
    pub parent_l0_pa: u64,
    /// Slot index whose `parent_frame` snapshot the handler should
    /// copy back over the current trap frame. Indexes
    /// [`super::PROCESS_TABLE`].
    pub parent_frame_slot: usize,
    /// Number of MDs `free_all_owned` released across both pools —
    /// useful for the D6 selftest assert and for Phase 06 D3
    /// `Pexec` stat reporting.
    pub freed_mds: usize,
}

/// Failure modes for [`terminate`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PtermError {
    /// `RUN` is null — `Pterm` was called from a path that has no
    /// running user process. Indicates a kernel bug (a real source
    /// port can only reach `Pterm` from EL0, where `RUN` is by
    /// construction non-null).
    NoRunningProcess,
    /// `RUN`'s basepage VA does not correspond to any
    /// [`super::PROCESS_TABLE`] slot. Indicates table corruption
    /// (the loader / `Pexec` failed to wire `basepage_kva`).
    NoSlotForRun,
    /// The slot's `parent_resume_pc` is `0` — the loader / `Pexec`
    /// dropped to EL0 without first capturing the parent context
    /// (see [`crate::bdos::process::Process::parent_resume_pc`] doc).
    NoParentResumeContext,
}

/// Tear down the running process's *resources* and return the
/// metadata the kernel SVC handler needs to finish the round-trip
/// (frame copy + TTBR0 restore + slot release).
///
/// **Slot is not freed here.** The handler must call
/// [`release_slot`] after it has copied the slot's
/// [`super::ParentFrame`] back over the current trap frame —
/// otherwise the slot's atomics would be zeroed before the handler
/// could read them.
///
/// `_rc` is preserved for spec parity but unused — the handler
/// writes the exit code into the destination frame's `gpr[0]`
/// directly because that is where the parent reads its `Pexec`
/// return value.
///
/// # Safety
///
/// Caller must hold the BDOS giant lock for the duration. The lock
/// guarantees no concurrent reader of `RUN`, the slot's atomics, or
/// the BDOS pool free lists. Caller must additionally guarantee
/// that the running process is the one that just trapped — i.e.
/// this is invoked from the synchronous SVC handler with the SVC's
/// trap frame still on the kernel stack.
pub unsafe fn terminate(_rc: i32) -> Result<PtermResume, PtermError> {
    let bp = RUN.load(Ordering::Acquire);
    if bp.is_null() {
        return Err(PtermError::NoRunningProcess);
    }
    let slot_idx = slot_for(bp).ok_or(PtermError::NoSlotForRun)?;
    let slot = &super::PROCESS_TABLE[slot_idx];

    if slot.parent_frame.elr().load(Ordering::Acquire) == 0 {
        return Err(PtermError::NoParentResumeContext);
    }
    let parent_l0_pa = slot.parent_l0_pa.load(Ordering::Acquire);

    // Sweep both BDOS pools for blocks owned by this process.
    // SAFETY: caller holds the BDOS giant lock; `bp` is a valid
    // BasePage pointer (slot lookup just confirmed it).
    let freed = unsafe { free_all_owned(bp as *mut c_void) };

    // EmuTOS `xterm` (bdos/proc.c:609): `run = run->p_parent`. On
    // child termination the **parent** becomes the running process
    // again, so the parent's own subsequent `Pterm` / BDOS calls see
    // a valid `RUN`. A boot-launched process (no user parent —
    // `parent_slot == NO_PARENT`, the Phase-06 D6 path) clears `RUN`
    // to null instead; the kernel resume path takes over there.
    //
    // The slot's other atomics stay populated until the handler calls
    // `release_slot` — it still needs `parent_frame` for the frame
    // copy.
    let parent_slot = slot.parent_slot.load(Ordering::Acquire);
    if parent_slot == super::NO_PARENT {
        set_current(ptr::null_mut::<BasePage>());
    } else {
        let parent_bp = super::PROCESS_TABLE[parent_slot as usize]
            .basepage_kva
            .load(Ordering::Acquire);
        set_current(parent_bp);
    }

    Ok(PtermResume {
        parent_l0_pa,
        parent_frame_slot: slot_idx,
        freed_mds: freed,
    })
}

/// Free the slot *after* the handler has copied
/// `PROCESS_TABLE[slot_idx].parent_frame` back over the current
/// trap frame. Released slots are reclaimable by the next
/// [`super::allocate_slot`] call.
///
/// # Safety
///
/// Caller must hold the BDOS giant lock and must have already
/// consumed the slot's `parent_frame` snapshot.
pub unsafe fn release_slot(slot_idx: usize) {
    free_slot(slot_idx);
}

/// `Pterm0` — equivalent to `Pterm(0)`. Spec:
/// [`docs/api/gemdos/process/00-pterm0.md`].
///
/// Exists as a distinct GEMDOS function ID (0x00) but has identical
/// semantics to `Pterm` with `rc = 0`.
///
/// # Safety
///
/// Same as [`terminate`].
pub unsafe fn terminate_zero() -> Result<PtermResume, PtermError> {
    // SAFETY: forwarded.
    unsafe { terminate(0) }
}
