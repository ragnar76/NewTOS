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
//!     return them to both pools' free lists (`Ptermres`: keep them
//!     resident instead, owned by the TSR holder).
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
//!   * `dirtbl[]` decrement — NewTOS keeps no per-PD current directories.
//!
//! The file half of `ixterm` (`p_uft[]` redirections and owned `sft[]`
//! entries, `bdos/proc.c:116-123`) runs since TASK/PROC02
//! ([`crate::bdos::fs::file::close_process_files`]).
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

use crate::bdos::mem::free_all_owned::{free_all_owned, reserve_all_owned};

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

/// The exit code `rc` as the terminated process's parent receives it — EmuTOS
/// `xterm(UWORD rc)` stores `run->p_dreg[0] = rc` (`bdos/proc.c:601,616`):
/// the low 16 bits, **zero-extended** into the LONG `Pexec` returns (D-005).
/// `Pterm(-1)` → 65535, `Pterm(0x12345)` → 0x2345, the console Ctrl-C's `-32`
/// → 65504. Never negative, so never confused with a `Pexec` load error.
///
/// Used for the kernel-launched child's exit code (the shell's `dos_exec`
/// value, PROC05); the `Pterm` trap applies the same mask to `x0` itself.
pub fn parent_exit_code(rc: i32) -> i32 {
    i32::from(rc as u16)
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
    // SAFETY: forwarded — same BDOS-lock / trap-context contract.
    unsafe { terminate_inner(false) }
}

/// `Ptermres` (GEMDOS 0x31) — terminate but leave the process's memory
/// **resident** (the TSR trick). Spec:
/// [`docs/api/gemdos/process/31-ptermres.md`]. EmuTOS `xtermres`
/// (`bdos/proc.c:624-634`): `xsetblk(0, run, blkln)` — the TPA block,
/// whose address is the PD, shrinks to `blkln` bytes, the result ignored
/// (D-006) — then the owned blocks are reserved, then `xterm`.
///
/// Every block the process owns — the shrunk TPA, the environment, its
/// `Malloc` blocks — stays allocated, owned by the TSR holder
/// ([`reserve_all_owned`], FreeMiNT's `TSR_Q`, plan §3.8), so resident
/// code and data stay where they are and callable from every process (one
/// shared address space). The exit-code / parent-resume path is identical
/// to [`terminate`].
///
/// # Safety
///
/// Same as [`terminate`].
pub unsafe fn terminate_resident(blkln: i32, _rc: i32) -> Result<PtermResume, PtermError> {
    let bp = RUN.load(Ordering::Acquire);
    if !bp.is_null() {
        // `xsetblk(0, run, blkln)`: same rules as `Mshrink` (len 0 frees
        // the block, a larger len is EGSBF) — and like EmuTOS, the answer
        // does not matter.
        // SAFETY: BDOS giant lock held (caller contract).
        let _ = unsafe { crate::bdos::mem::traps::mshrink(0, bp as u64 as tos_types::Long, blkln) };
    }
    // SAFETY: forwarded.
    unsafe { terminate_inner(true) }
}

/// Shared body of [`terminate`] / [`terminate_resident`]. `reserve`
/// selects whether the dying process's owned pool blocks are freed
/// (`false`, `Pterm`) or kept resident (`true`, `Ptermres`).
///
/// # Safety
///
/// See [`terminate`].
unsafe fn terminate_inner(reserve: bool) -> Result<PtermResume, PtermError> {
    // AES trap tracer: write the trace to `C:` before the process is torn
    // down, so the file survives the application exiting. Runs on the kernel
    // exception stack (this is called from `rust_sync_handler`'s `Pterm`
    // path), never the 16 KiB AES fiber stack. Compiled out entirely without
    // the `aes-trace` feature.
    crate::aes::trace::flush_on_exit();
    // GEMDOS trap tracer: same reason — the file-system conversation is only
    // complete once the process that had it is gone, and this is the last
    // moment at which it can still be written.
    crate::gemdos::trace::flush_on_exit();

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

    // AES resource lists: drop the dying process's entries (A39, R6). EmuTOS
    // needs no such step — its resource state is the three words in the
    // application's own `AESGLOBAL`, which die with the TPA, and the blob is
    // one of the owned blocks `free_all_owned` below reclaims (`ixterm`,
    // `bdos/proc.c:136`). NewTOS keeps a per-slot descriptor in the AES, and
    // the next program in this slot has the same AES identity (the slot) and,
    // at `appl_init`, the same AES descriptor — so the entries must go here,
    // on both the `Pterm` and the `Ptermres` path, or the next tenant would
    // inherit kernel aliases of memory that is no longer its resource. Only
    // this slot's entries; the parent's stay. The pool blocks are handled by
    // the sweep below (or kept resident by `Ptermres`).
    crate::aes::resource::on_process_exit(slot_idx);

    // `ixterm` (`bdos/proc.c:116-123`): close the standard handles it
    // redirected to files, then every `sft[]` entry it owns — before its
    // memory goes, as EmuTOS orders it (TASK/PROC02).
    // SAFETY: BDOS lock held; `bp` is the dying process's live basepage.
    unsafe { crate::bdos::fs::file::close_process_files(bp) };

    // The sweep and the release of unstarted children are one change of
    // rights, entered when this guard drops (MEMPROT-FAST): nothing in
    // between runs at EL0 or touches user memory through the user tables.
    // SAFETY: caller holds the BDOS giant lock.
    let batch = unsafe { super::memprot::batch() };
    // Sweep both BDOS pools for blocks owned by this process: free
    // them (`Pterm`) or reserve them as resident (`Ptermres`).
    // SAFETY: caller holds the BDOS giant lock; `bp` is a valid
    // BasePage pointer (slot lookup just confirmed it).
    let freed = unsafe {
        if reserve {
            reserve_all_owned(bp as *mut c_void)
        } else {
            free_all_owned(bp as *mut c_void)
        }
    };
    // Programs this process loaded with `PE_LOAD` (or built with `Pexec`
    // 5/7) and never started lived in its blocks (EmuTOS `owner = run`);
    // their slots go with it — on `Ptermres` too: their memory stays
    // resident with the rest, but no process is left that could start them.
    super::pexec::release_unstarted_of(bp);
    drop(batch);

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
        set_current(super::pd_of(parent_slot as usize));
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
    // Drop the record that this process held an uncached screen pointer (a
    // `Physbase`/`Logbase` framebuffer address), so the VDI's per-primitive
    // flush returns to the cheaper clean-only form once no program is
    // drawing into the screen itself (`super::usermem::uncached_shared_active`).
    super::usermem::clear_uncached(slot_idx);
    // The slot's AES process descriptor is keyed by the slot (A62): clear its
    // flags and drain its queue, so the next tenant does not inherit a dead
    // program's `AP_OPEN` or unread messages through `appl_init`'s
    // preserve-the-queue rule. This covers every termination path, including
    // an EL0 `Pexec` child the launcher's `sh_ldapp` cleanup never sees.
    crate::aes::release_client(slot_idx);
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
