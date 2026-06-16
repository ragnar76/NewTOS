//! BDOS process table — Phase 06 D2.
//!
//! Mirrors EmuTOS `bdos/proc.c` runtime state: a fixed-size table of
//! per-process kernel slots plus the two distinguished pointers
//! `run` (currently-executing process) and `nextrun` (process queued
//! for the next trap-return / Pexec hand-off).
//!
//! ## Scope of this deliverable
//!
//! D2 introduces only the **data structures** the rest of Phase 06
//! builds on:
//!
//!   * [`Process`] — the kernel-side slot wrapping the user-visible
//!     [`BasePage`].
//!   * [`MAX_PROCESSES`] — bound on concurrent processes (16 for now,
//!     plenty for normal Atari-style sequential `Pexec` chains).
//!   * [`PROCESS_TABLE`] — static array of slots in BSS.
//!   * [`RUN`] / [`NEXTRUN`] — `AtomicPtr<BasePage>` for lock-free
//!     reads from the SVC handler.
//!   * Allocation / free / lookup helpers for the slots.
//!
//! The slot's per-process L0 page-table phys (`l0_pa`) is reserved
//! but not yet populated — Phase 06 D8 fills it during the per-
//! process TTBR0 install. ELF loading (D3), `Pexec` modes (D4/D5),
//! `Pterm` cleanup (D6) and `m_own` integration (D9) all consume
//! this table without altering its shape.
//!
//! ## Concurrency model
//!
//! Each [`Process`] field is atomic so the SVC sync handler — which
//! runs without taking the BDOS giant lock — can read the running
//! process's basepage in a single relaxed-acquire load. Slot
//! allocation uses a CAS on the `state` field so two concurrent
//! `Pexec` calls (Phase-08+ scenario) don't claim the same slot.
//! All other writes happen under the BDOS giant lock that the
//! GEMDOS dispatcher implicitly holds (Phase 06 D6 will replace the
//! "implicit single-CPU" lock with an explicit per-CPU SpinMutex).

use core::ptr;
use core::sync::atomic::{AtomicI32, AtomicPtr, AtomicU32, AtomicU64, AtomicU8, Ordering};

use tos_types::BasePage;

pub mod arena;
pub mod elf;
pub mod loader;
pub mod mmu;
pub mod pterm;
pub mod pexec;
pub mod registry;

// ─── Bounds + state encoding ────────────────────────────────────────

/// Maximum number of concurrently-tracked processes. EmuTOS does not
/// bound this explicitly (it relies on memory pressure to fail
/// `Pexec`); we pick a small static cap because TOS process chains
/// are sequential and shallow — even AES + a desktop accessory + a
/// running app rarely exceeds five live processes.
pub const MAX_PROCESSES: usize = 16;

/// Sentinel value for `parent_slot` indicating "no parent" (the boot
/// process loaded by the kernel itself).
pub const NO_PARENT: u32 = u32::MAX;

/// Process-slot lifecycle. Encoded as `u8` so the field can sit in
/// an `AtomicU8` for lock-free state probes.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum State {
    /// Slot is unused; safe to claim via [`allocate_slot`].
    Free = 0,
    /// Slot is reserved, basepage allocated, but the program has not
    /// yet started executing. Set by `Pexec` mode 3 (`PE_LOAD`) and
    /// by mode 0 (`PE_LOADGO`) before the first eret.
    Loaded = 1,
    /// Slot is the active process. Equals [`RUN`]'s target.
    Running = 2,
    /// `Pterm`-marked but not yet reaped — its allocations have not
    /// been freed by `free_all_owned` yet. Transient.
    Terminated = 3,
}

impl State {
    /// Decode a raw `u8` from [`Process::state`]. Defensive: any
    /// value outside `0..=3` is treated as `Free`.
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Loaded,
            2 => Self::Running,
            3 => Self::Terminated,
            _ => Self::Free,
        }
    }
}

// ─── Process slot ───────────────────────────────────────────────────

/// Per-process kernel state. Sits next to (but not inside) the
/// user-visible [`BasePage`]: applications never see this struct, so
/// it can carry kernel-only fields (page-table root, parent slot
/// index, etc.) without touching the bit-exact 256-byte basepage
/// ABI.
///
/// All fields are atomic so the SVC handler can probe them lock-free.
/// Mutations during `Pexec` / `Pterm` happen under the BDOS giant
/// lock, but readers don't need to acquire it.
pub struct Process {
    /// Slot lifecycle (see [`State`]).
    pub state: AtomicU8,

    /// Kernel-side virtual address of the [`BasePage`] living at the
    /// start of this process's TPA. Always a high-half VA; the value
    /// has the same bits as the corresponding low-half user VA modulo
    /// `VA_OFFSET`.
    pub basepage_kva: AtomicPtr<BasePage>,

    /// Physical address of the [`BasePage`]. Stored separately so
    /// the page-table builder (Phase 06 D8) can install the user
    /// mapping without re-walking the kernel translation regime.
    pub basepage_pa: AtomicU64,

    /// Phys of the per-process L0 page-table root that gets written
    /// to `TTBR0_EL1` when this process becomes [`State::Running`].
    /// `0` until Phase 06 D8 builds the tables for this process.
    pub l0_pa: AtomicU64,

    /// User VA of the program's entry point (the ELF `e_entry`). The
    /// loader records it so `Pexec(PE_GO)` can launch a slot that was
    /// loaded earlier by `Pexec(PE_LOAD)` without re-parsing the image.
    pub entry_va: AtomicU64,

    /// Index of the parent's slot in [`PROCESS_TABLE`], or
    /// [`NO_PARENT`] for the boot-loaded root process. Used by
    /// `Pterm` to return control to the parent's resume PC.
    pub parent_slot: AtomicU32,

    /// Snapshot of the parent's trap frame. Pterm copies this back
    /// over the child's trap frame so the post-handler `eret`
    /// resumes the parent at exactly the state it was in when it
    /// dispatched the call that led to this child.
    ///
    /// Two writers populate this:
    ///   * **`Pexec` (real EL0 parent)** — copies the parent's
    ///     trap frame here verbatim before redirecting EL0 to the
    ///     child. The eventual `Pterm` restores the parent at its
    ///     original EL0 PC (the SVC's resume point) with `x0 = rc`.
    ///   * **Kernel-launched test driver (D6 path)** — writes only
    ///     the resume PC, kernel SP, and `SPSR.M = EL1t` via the
    ///     [`crate::bdos::process::loader`]/EL0-entry asm. The
    ///     gpr / lr / spsr.flags fields stay zero; the kernel
    ///     resume code reads its callee-saved regs from the local
    ///     stack frame so junk in `frame.gpr` is harmless.
    pub parent_frame: ParentFrame,

    /// Phys address of the parent's `L0` page-table root. Pterm
    /// installs this in `TTBR0_EL1` (with a TLBI, since we run in
    /// kernel context not eret-adjacent) before resuming the parent.
    /// `0` means "no parent process" — boot is the parent, and the
    /// handler restores the Stage-1 boot tables instead.
    ///
    /// Phase 06 D6 always sees `0` here (boot launches the test
    /// process directly). Phase 06 D3 `Pexec` populates it with the
    /// parent's TTBR0_EL1 read at trap entry, so the child's
    /// `Pterm` brings the parent's address space back online.
    pub parent_l0_pa: AtomicU64,
}

// ─── Parent-frame snapshot ──────────────────────────────────────────

/// Index of `lr` (`x30`) in [`ParentFrame::regs`].
pub const PARENT_FRAME_LR: usize = 30;
/// Index of `elr_el1` in [`ParentFrame::regs`].
pub const PARENT_FRAME_ELR: usize = 31;
/// Index of `spsr_el1` in [`ParentFrame::regs`].
pub const PARENT_FRAME_SPSR: usize = 32;
/// Index of `sp_el0` in [`ParentFrame::regs`].
pub const PARENT_FRAME_SP_EL0: usize = 33;
/// Total number of `u64` slots in [`ParentFrame`] — `30` for
/// `x0..x29` plus four one-off slots above.
pub const PARENT_FRAME_LEN: usize = 34;

/// Atomic snapshot of an [`ExceptionFrame`]'s restorable bits
/// (everything the post-handler `eret` consumes). Layout matches
/// [`crate::bdos::process::PARENT_FRAME_*`] index constants.
///
/// All reads/writes go through atomics for the same single-CPU
/// reason as the rest of [`Process`]; the slot is mutated only
/// under the BDOS giant lock, but other slot fields are atomic
/// anyway and a non-atomic outlier would break the auto-`Sync`
/// derivation.
#[repr(C)]
pub struct ParentFrame {
    /// `x0..x29` (= 30 slots), followed by `lr`, `elr`, `spsr`,
    /// `sp_el0`. Use [`Self::gpr`], [`Self::lr`], [`Self::elr`],
    /// [`Self::spsr`], [`Self::sp_el0`] for named access.
    pub regs: [AtomicU64; PARENT_FRAME_LEN],
}

impl ParentFrame {
    /// All-zero. Sentinel value: `elr() == 0` means "no parent
    /// frame captured" (slot is fresh / freed).
    #[inline]
    pub const fn zeroed() -> Self {
        Self {
            regs: [const { AtomicU64::new(0) }; PARENT_FRAME_LEN],
        }
    }

    /// `&AtomicU64` for a general-purpose register. `i` must be in
    /// `0..30`.
    #[inline]
    pub fn gpr(&self, i: usize) -> &AtomicU64 {
        &self.regs[i]
    }

    /// `&AtomicU64` for `lr` (`x30`).
    #[inline]
    pub fn lr(&self) -> &AtomicU64 {
        &self.regs[PARENT_FRAME_LR]
    }

    /// `&AtomicU64` for `elr_el1`.
    #[inline]
    pub fn elr(&self) -> &AtomicU64 {
        &self.regs[PARENT_FRAME_ELR]
    }

    /// `&AtomicU64` for `spsr_el1`.
    #[inline]
    pub fn spsr(&self) -> &AtomicU64 {
        &self.regs[PARENT_FRAME_SPSR]
    }

    /// `&AtomicU64` for `sp_el0`.
    #[inline]
    pub fn sp_el0(&self) -> &AtomicU64 {
        &self.regs[PARENT_FRAME_SP_EL0]
    }

    /// Reset every slot to zero. Called by [`free_slot`] when a
    /// slot is released.
    #[inline]
    pub fn clear(&self) {
        for r in self.regs.iter() {
            r.store(0, Ordering::Release);
        }
    }
}

impl Process {
    /// All-zero / "Free" slot. The `const fn` form lets us
    /// `[const { Process::empty() }; MAX_PROCESSES]` in static-array
    /// initialisation.
    #[inline]
    pub const fn empty() -> Self {
        Self {
            state: AtomicU8::new(State::Free as u8),
            basepage_kva: AtomicPtr::new(ptr::null_mut()),
            basepage_pa: AtomicU64::new(0),
            l0_pa: AtomicU64::new(0),
            entry_va: AtomicU64::new(0),
            parent_slot: AtomicU32::new(NO_PARENT),
            parent_frame: ParentFrame::zeroed(),
            parent_l0_pa: AtomicU64::new(0),
        }
    }

    /// Read the current lifecycle state.
    #[inline]
    pub fn read_state(&self) -> State {
        State::from_u8(self.state.load(Ordering::Acquire))
    }

    /// Set the lifecycle state. Caller must own the slot (i.e. have
    /// claimed it via [`allocate_slot`] or already be transitioning
    /// it under the BDOS giant lock).
    #[inline]
    pub fn set_state(&self, s: State) {
        self.state.store(s as u8, Ordering::Release);
    }
}

// ─── Static table + run/nextrun ────────────────────────────────────

/// Kernel-wide process table. 16 slots × ~24 bytes each = ~384 B in
/// BSS — well below any meaningful overhead.
pub static PROCESS_TABLE: [Process; MAX_PROCESSES] =
    [const { Process::empty() }; MAX_PROCESSES];

/// Pointer to the [`BasePage`] of the currently-running process, or
/// `null` while only kernel code is on the stack (boot, between
/// `Pterm` and the next `Pexec`). Read by the SVC sync handler to
/// stamp ownership on new BDOS allocations (Phase 06 D9 wires this
/// into [`crate::bdos::mem::traps::current_owner`]).
pub static RUN: AtomicPtr<BasePage> = AtomicPtr::new(ptr::null_mut());

/// Pointer to the [`BasePage`] of the process that should run next
/// after the current trap returns. Set by `Pexec` modes that load
/// without immediately entering EL0 (`PE_LOAD`); consumed by the
/// trap-exit path which moves it into [`RUN`] just before `eret`.
pub static NEXTRUN: AtomicPtr<BasePage> = AtomicPtr::new(ptr::null_mut());

/// Pointer to the currently-running [`BasePage`], or `null` if no
/// user process is active. Used by [`crate::bdos::mem::traps::current_owner`]
/// (Phase 06 D9) and by the GEMDOS dispatcher when consulting
/// `run->p_uft[…]` standard-handle-redirection (Phase 09).
#[inline]
pub fn current() -> *mut BasePage {
    RUN.load(Ordering::Acquire)
}

/// Set the running-process pointer. Atomic so the SVC handler that
/// reads [`current`] sees a consistent value at every moment.
#[inline]
pub fn set_current(bp: *mut BasePage) {
    RUN.store(bp, Ordering::Release);
}

// ─── Deferred terminate (Ctrl-C from blocking console input) ────────

/// Sentinel meaning "no terminate pending". `i32::MIN` is never a
/// valid TOS exit code (rc is a WORD), so it cannot collide.
const NO_PENDING_TERMINATE: i32 = i32::MIN;

/// Exit code of a pending self-terminate request, or
/// [`NO_PENDING_TERMINATE`].
static PENDING_TERMINATE: AtomicI32 = AtomicI32::new(NO_PENDING_TERMINATE);

/// Request that the **current** process self-terminate with exit code
/// `rc` once the current SVC returns. The blocking console input
/// functions use this for Ctrl-C (`rc = -32`, EmuTOS `terminate()`):
/// a leaf GEMDOS function returns a value in `x0` and cannot redirect
/// the trap frame to the parent, so it records the request here and
/// the kernel SVC handler performs the Pterm-style redirect via
/// [`take_pending_terminate`].
pub fn request_terminate(rc: i32) {
    PENDING_TERMINATE.store(rc, Ordering::Release);
}

/// Consume a pending self-terminate request set by
/// [`request_terminate`], returning its exit code if one is pending.
pub fn take_pending_terminate() -> Option<i32> {
    let rc = PENDING_TERMINATE.swap(NO_PENDING_TERMINATE, Ordering::AcqRel);
    if rc == NO_PENDING_TERMINATE {
        None
    } else {
        Some(rc)
    }
}

/// The "queued for next eret" basepage, or `null`.
#[inline]
pub fn pending() -> *mut BasePage {
    NEXTRUN.load(Ordering::Acquire)
}

/// Set the queued-next basepage.
#[inline]
pub fn set_pending(bp: *mut BasePage) {
    NEXTRUN.store(bp, Ordering::Release);
}

// ─── Slot allocation / lookup ──────────────────────────────────────

/// Claim the first free slot. CAS-based so concurrent `Pexec`s
/// don't double-claim. Returns the slot index, or `None` when the
/// table is full (`Pexec` should then return `ENSMEM`).
///
/// The claimed slot is left in [`State::Loaded`]; the caller fills
/// in the basepage / parent / `l0_pa` fields before transitioning
/// to [`State::Running`].
pub fn allocate_slot() -> Option<usize> {
    for (i, p) in PROCESS_TABLE.iter().enumerate() {
        // Try to flip Free → Loaded. Stronger compare_exchange
        // because we don't want spurious failures on platforms
        // where weak CAS may fail without contention.
        if p.state
            .compare_exchange(
                State::Free as u8,
                State::Loaded as u8,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            return Some(i);
        }
    }
    None
}

/// Release a slot, zeroing every field. Paired with
/// [`allocate_slot`] (and the late-`Pterm` cleanup that runs
/// `free_all_owned` against this slot's basepage pointer first).
pub fn free_slot(idx: usize) {
    if idx >= MAX_PROCESSES {
        return;
    }
    let p = &PROCESS_TABLE[idx];
    p.basepage_kva.store(ptr::null_mut(), Ordering::Release);
    p.basepage_pa.store(0, Ordering::Release);
    p.l0_pa.store(0, Ordering::Release);
    p.entry_va.store(0, Ordering::Release);
    p.parent_slot.store(NO_PARENT, Ordering::Release);
    p.parent_frame.clear();
    p.parent_l0_pa.store(0, Ordering::Release);
    // State last so that any concurrent reader sees `Free` only once
    // every other field is observably reset.
    p.state.store(State::Free as u8, Ordering::Release);
}

/// Find the slot holding `bp`, if any. Linear scan — the table is
/// small and the lookup runs at most once per `Pterm` / per per-
/// process trap, so the cost is negligible.
pub fn slot_for(bp: *mut BasePage) -> Option<usize> {
    if bp.is_null() {
        return None;
    }
    for (i, p) in PROCESS_TABLE.iter().enumerate() {
        if p.read_state() != State::Free
            && p.basepage_kva.load(Ordering::Acquire) == bp
        {
            return Some(i);
        }
    }
    None
}

/// Borrow a slot by index.
#[inline]
pub fn slot(idx: usize) -> Option<&'static Process> {
    PROCESS_TABLE.get(idx)
}
