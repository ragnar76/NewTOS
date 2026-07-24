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
use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicPtr, AtomicU32, AtomicU64, AtomicU8, Ordering,
};

use tos_types::BasePage;

pub mod arena;
pub mod elf;
pub mod loader;
pub mod mmu;
pub mod pterm;
pub mod pexec;
pub mod registry;
pub mod sched;
pub mod usermem;

// ─── Bounds + state encoding ────────────────────────────────────────

/// Maximum number of concurrently-tracked processes. EmuTOS does not
/// bound this explicitly (it relies on memory pressure to fail
/// `Pexec`); we pick a small static cap because TOS process chains
/// are sequential and shallow — even AES + a desktop accessory + a
/// running app rarely exceeds five live processes.
pub const MAX_PROCESSES: usize = 16;

/// Maximum EL0-upcall nesting depth per process (cooperative-kernel
/// building block 1, `docs/analysis/coop-kernel/10-upcall-signal-design.md`
/// §6.3). Bounds the per-slot saved-context stack [`Process::upcall_ctx`];
/// a delivery that would exceed it terminates the process with
/// `Pterm(-998)` — the FreeMiNT "stack overflow" ALERT analogue
/// (`references/freemint/sys/arch/sig_mach.c:138`). FreeMiNT instead bounds
/// nesting by its 16-KiB system stack (~12 B/nest, `sig_mach.c:141-144`);
/// NewTOS bounds it by this array. Memory: `ParentFrame` × 8 × 16 slots
/// ≈ 34 KiB BSS (design §6.3).
pub const MAX_UPCALL_NEST: usize = 8;

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

// ─── Per-process ABI (pointer / `long` width) ───────────────────────

/// Application binary interface of a loaded program, derived from its
/// ELF class at load time (ADR-020, the ILP32 pivot). It is the width
/// of a C pointer and `long` in the program's address space, and thus
/// how the shared GEM trap (`svc #2`) must interpret the AES/VDI
/// parameter block the app hands in: an ILP32 app builds the block from
/// **4-byte** pointers, an LP64 app from **8-byte** pointers. The WORD
/// arrays the pointers point at are 16-bit either way; only the pointer
/// reads (the six/five block pointers, and any `long`/pointer inside the
/// AES `addrin`/`addrout` arrays) change width.
///
/// Encoded as `u8` so it can live in an [`AtomicU8`] alongside the rest
/// of the lock-free [`Process`] slot state.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Abi {
    /// Native AArch64 LP64: 8-byte pointers and `long`. Kernel-launched
    /// ELF64 programs, the embedded selftests, and every in-kernel (EL1)
    /// GEM caller (the desktop, the boot VDI/AES selftests) use this.
    Lp64 = 0,
    /// GNU ILP32 (`aarch64-elf-gcc -mabi=ilp32`, `ELFCLASS32`): 4-byte
    /// pointers and `long`. Source-port GEM apps built against the ILP32
    /// SDK (libc-ilp32 / gemlib-ilp32 / cflib-ilp32) run in this ABI.
    Ilp32 = 1,
}

impl Abi {
    /// Decode a raw `u8` from [`Process::abi`]. Defensive: any value
    /// other than `1` (`Ilp32`) is treated as `Lp64`, so a fresh/zeroed
    /// slot reads as the native ABI.
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Ilp32,
            _ => Self::Lp64,
        }
    }

    /// Read the `i`-th pointer-width word from a parameter-block array
    /// based at `base`, widened to `u64`: an 8-byte load at 8-byte stride
    /// for [`Abi::Lp64`], a 4-byte (zero-extended) load at 4-byte stride
    /// for [`Abi::Ilp32`]. Used for both the AES/VDI block pointers and
    /// the AES `addrin` `long`/pointer elements (a `long` is exactly one
    /// pointer wide in each ABI).
    ///
    /// # Safety
    /// `base + i * width` must address a valid, readable slot.
    #[inline]
    pub unsafe fn read_ptr(self, base: u64, i: usize) -> u64 {
        match self {
            // SAFETY: forwarded to the caller's bounds contract.
            Self::Lp64 => unsafe { (base as *const u64).add(i).read() },
            // SAFETY: forwarded to the caller's bounds contract.
            Self::Ilp32 => unsafe { (base as *const u32).add(i).read() as u64 },
        }
    }

    /// Write `v` into the `i`-th pointer-width slot of a parameter-block
    /// array based at `base` (the inverse of [`Self::read_ptr`]): an
    /// 8-byte store for [`Abi::Lp64`], the low 4 bytes for [`Abi::Ilp32`].
    /// Used by the AES `addrout` writers (`rsrc_gaddr`, …).
    ///
    /// # Safety
    /// `base + i * width` must address a valid, writable slot.
    #[inline]
    pub unsafe fn write_ptr(self, base: u64, i: usize, v: u64) {
        match self {
            // SAFETY: forwarded to the caller's bounds contract.
            Self::Lp64 => unsafe { (base as *mut u64).add(i).write(v) },
            // SAFETY: forwarded to the caller's bounds contract.
            Self::Ilp32 => unsafe { (base as *mut u32).add(i).write(v as u32) },
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

    /// Program ABI (pointer / `long` width), recorded from the ELF class
    /// by the loader (ADR-020). Read lock-free by the GEM trap dispatch
    /// via [`current_abi`] so the AES/VDI parameter-block readers use the
    /// caller's pointer width. Encoded as `u8` per [`Abi`]; a zeroed slot
    /// reads as [`Abi::Lp64`].
    pub abi: AtomicU8,

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

    // ── cooperative scheduler (Phase 06 EC4 / `sched`) ──
    /// Full EL0 context saved when this process yields via `Pwait`
    /// (so the round-robin can resume it exactly where it left off).
    /// Distinct from [`Self::parent_frame`], which carries the
    /// parent-resume linkage for `Pexec`/`Pterm`.
    pub saved_frame: ParentFrame,

    /// In the scheduler's run-set (a peer in the cooperative
    /// round-robin). Cleared when the process terminates.
    pub runnable: AtomicBool,

    /// `saved_frame` holds a valid context (the process has run and
    /// yielded). `false` = never entered → start fresh at `entry_va`.
    pub has_ctx: AtomicBool,

    /// Wait-condition key when this process is **WAITING** — cooperative-kernel
    /// step 3a (sleep/wake substrate, `docs/analysis/coop-kernel/`
    /// `20-sleepwake-substrate-design.md` §3.1/§3.2/§4.1). `0` = not waiting
    /// (the process is RUNNING or READY). A nonzero value is a wake-class key
    /// built by [`sched::wait_input`]/[`sched::wait_mesg`]/[`sched::wait_timer`]
    /// (class in the high bits, pid/deadline below, §8.1.3): the process sleeps
    /// in [`sched::sleep`] until [`sched::wake`] — or the idle wake-pass — clears
    /// it (WAITING → READY). Layered over the existing [`Self::runnable`] bit,
    /// not a separate list: WAITING is `runnable == false && wait_cond != 0`,
    /// READY is `runnable == true` (§3.2 "field, not list").
    ///
    /// [`sched::sleep`]: super::process::sched::sleep
    /// [`sched::wake`]: super::process::sched::wake
    /// [`sched::wait_input`]: super::process::sched::wait_input
    /// [`sched::wait_mesg`]: super::process::sched::wait_mesg
    /// [`sched::wait_timer`]: super::process::sched::wait_timer
    pub wait_cond: AtomicU64,

    // ── cooperative-kernel EL0 upcall / signal delivery (building block 1,
    //    docs/analysis/coop-kernel/10-upcall-signal-design.md §6) ──
    /// Kernel-side stack of interrupted EL0 contexts, one per active
    /// (possibly nested) upcall. `deliver_upcall` pushes the pre-upcall
    /// frame here (with the syscall result already in `gpr[0]`, §6.2);
    /// `Psigreturn` (GEMDOS 0x11A) pops + restores it. Bounded by
    /// [`MAX_UPCALL_NEST`]. Kernel-side rather than on the EL0 stack (design
    /// §6.1: an EL0 app cannot corrupt the resume context). Styled on
    /// [`Self::saved_frame`].
    pub upcall_ctx: [ParentFrame; MAX_UPCALL_NEST],

    /// Mask bit each [`Self::upcall_ctx`] level was delivered with, so
    /// `Psigreturn` clears exactly the bit its level set. FreeMiNT recovers
    /// the signal number from the on-stack frame (`sig_mach.c:277,291`); with
    /// the kernel-side context stack we key it off the nesting depth instead.
    pub upcall_ctx_mask: [AtomicU32; MAX_UPCALL_NEST],

    /// [`Disposition`] each [`Self::upcall_ctx`] level was delivered with
    /// (cooperative-kernel step 2), encoded per [`Disposition::encode`]. On
    /// `Psigreturn`, the top level's word decides resume (step-1 default,
    /// `0`) vs terminate-after (etv_term). A resume level always stores `0`,
    /// so this array is transparent to the step-1 resume path.
    pub upcall_ctx_disp: [AtomicU64; MAX_UPCALL_NEST],

    /// Current upcall nesting depth (= number of live [`Self::upcall_ctx`]
    /// entries). `deliver_upcall` increments, `Psigreturn` decrements.
    pub upcall_depth: AtomicU8,

    /// Set of upcall-type bits currently being handled — the FreeMiNT
    /// `p_sigmask` analogue (`sig_mach.c:291`). A type stays masked while its
    /// handler runs and is cleared on the matching `Psigreturn`.
    pub upcall_mask: AtomicU32,

    /// Pending (posted, not-yet-delivered) upcall handler VA, or `0` for
    /// none — the presence sentinel of the [`UpcallDesc`] posted by
    /// [`post_upcall`] and consumed by [`take_pending_upcall`] (the per-slot
    /// [`request_terminate`]/[`take_pending_terminate`] analogue, design §7.1).
    pub upcall_pending_handler: AtomicU64,

    /// Argument registers (x0..x3) of the pending upcall.
    pub upcall_pending_args: [AtomicU64; 4],

    /// Mask bit of the pending upcall.
    pub upcall_pending_mask: AtomicU32,

    /// [`Disposition`] of the pending upcall (cooperative-kernel step 2),
    /// encoded per [`Disposition::encode`]; `0` (= [`Disposition::Resume`])
    /// for step-1 consumers.
    pub upcall_pending_disp: AtomicU64,
}

/// A pending EL0 upcall: the handler VA to jump to, up to four
/// argument-register values (x0..x3, AAPCS64), the mask bit identifying
/// the upcall type (set on delivery, cleared by `Psigreturn`), and the
/// [`Disposition`] on handler return. The consumers (etv_*, vex,
/// G_USERDEF, eralert, …) supply the handler VA directly — this is
/// disposition-agnostic in the POSIX sense (no signal-number table,
/// design §1), but each delivery carries its own resume-vs-terminate
/// [`Disposition`]. Analogue of a FreeMiNT signal resolved to its
/// `sa_handler` (`sig_mach.c`).
#[derive(Copy, Clone)]
pub struct UpcallDesc {
    /// EL0 VA of the handler the process runs before resuming.
    pub handler_va: u64,
    /// Handler arguments, delivered in x0..x3.
    pub args: [u64; 4],
    /// Upcall-type bit (masked while the handler runs).
    pub mask_bit: u32,
    /// What happens when the handler returns via `Psigreturn`
    /// (cooperative-kernel step 2). Step-1 consumers use
    /// [`Disposition::Resume`]; the etv_term consumer uses
    /// [`Disposition::Terminate`].
    pub disposition: Disposition,
}

/// What the kernel does when an EL0 upcall handler returns via `Psigreturn`
/// (cooperative-kernel step 2, `docs/analysis/coop-kernel/`
/// `10-upcall-signal-design.md`). Extends the step-1 mechanism, which only
/// ever resumed the interrupted context, with a "run the handler, then
/// terminate" disposition — the shape the **etv_term** process-terminate
/// hook needs (EmuTOS `xterm`, `bdos/proc.c:601-618`: `protect_v(userterm)`
/// runs the installed handler, then teardown continues to the parent).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Disposition {
    /// Restore the interrupted EL0 context and resume it (step-1 default).
    Resume,
    /// After the handler returns, terminate the process instead of resuming:
    /// route into the standard `Pterm` teardown with exit code `rc` and,
    /// for `Ptermres`, resident-block reservation (`reserve`). This is the
    /// etv_term "notify before I die" contract.
    Terminate {
        /// Original `Pterm` exit code (UWORD, already masked to 16 bits).
        rc: i32,
        /// `true` for `Ptermres` (0x31) — keep the process's owned pool
        /// blocks resident; `false` for `Pterm`/`Pterm0`.
        reserve: bool,
    },
}

impl Disposition {
    /// Tag bit marking a [`Disposition::Terminate`] in the `AtomicU64`
    /// encoding used by the pending / per-nest slots. A `0` word decodes to
    /// [`Disposition::Resume`], keeping the step-1 default a plain zero.
    const TERM_TAG: u64 = 1 << 63;
    /// `reserve` (Ptermres) flag in the `AtomicU64` encoding.
    const RESERVE_TAG: u64 = 1 << 62;

    /// Encode into the per-nest / pending `AtomicU64`. [`Disposition::Resume`]
    /// is `0`, so a freshly-zeroed slot means "resume".
    pub fn encode(self) -> u64 {
        match self {
            Disposition::Resume => 0,
            Disposition::Terminate { rc, reserve } => {
                Self::TERM_TAG
                    | if reserve { Self::RESERVE_TAG } else { 0 }
                    | (rc as u32 as u64)
            }
        }
    }

    /// Decode a word written by [`Self::encode`].
    pub fn decode(v: u64) -> Self {
        if v & Self::TERM_TAG == 0 {
            Disposition::Resume
        } else {
            Disposition::Terminate {
                rc: (v & 0xFFFF_FFFF) as u32 as i32,
                reserve: v & Self::RESERVE_TAG != 0,
            }
        }
    }
}

/// Upcall-type mask bit for the **etv_term** process-terminate hook (the
/// FreeMiNT `p_sigmask` per-consumer bit, `sig_mach.c:291`). Each future
/// upcall consumer (etv_critic, vex, …) claims its own bit; etv_term is the
/// first real one.
pub const UPCALL_MASK_ETV_TERM: u32 = 1 << 0;

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
            abi: AtomicU8::new(Abi::Lp64 as u8),
            basepage_kva: AtomicPtr::new(ptr::null_mut()),
            basepage_pa: AtomicU64::new(0),
            l0_pa: AtomicU64::new(0),
            entry_va: AtomicU64::new(0),
            parent_slot: AtomicU32::new(NO_PARENT),
            parent_frame: ParentFrame::zeroed(),
            parent_l0_pa: AtomicU64::new(0),
            saved_frame: ParentFrame::zeroed(),
            runnable: AtomicBool::new(false),
            has_ctx: AtomicBool::new(false),
            wait_cond: AtomicU64::new(0),
            upcall_ctx: [const { ParentFrame::zeroed() }; MAX_UPCALL_NEST],
            upcall_ctx_mask: [const { AtomicU32::new(0) }; MAX_UPCALL_NEST],
            upcall_ctx_disp: [const { AtomicU64::new(0) }; MAX_UPCALL_NEST],
            upcall_depth: AtomicU8::new(0),
            upcall_mask: AtomicU32::new(0),
            upcall_pending_handler: AtomicU64::new(0),
            upcall_pending_args: [const { AtomicU64::new(0) }; 4],
            upcall_pending_mask: AtomicU32::new(0),
            upcall_pending_disp: AtomicU64::new(0),
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

    /// Read the program ABI recorded for this slot.
    #[inline]
    pub fn read_abi(&self) -> Abi {
        Abi::from_u8(self.abi.load(Ordering::Acquire))
    }

    /// Record the program ABI for this slot (loader, at load time).
    #[inline]
    pub fn set_abi(&self, a: Abi) {
        self.abi.store(a as u8, Ordering::Release);
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

/// ABI (pointer / `long` width) of the currently-running process, or
/// [`Abi::Lp64`] when no user process owns the running basepage.
///
/// The GEM trap dispatch (`svc #2`) reads this to interpret the caller's
/// AES/VDI parameter block at the right pointer width. An EL0 app's slot
/// carries the ABI its ELF class implied; every in-kernel (EL1) GEM
/// caller — the desktop and the boot-time VDI/AES selftests, which run
/// while [`RUN`] is still null — resolves to [`Abi::Lp64`] and keeps its
/// native 64-bit blocks. (The desktop never reaches this path at all: it
/// calls the AES/VDI functions directly, not through the trap readers.)
#[inline]
pub fn current_abi() -> Abi {
    match slot_for(current()) {
        Some(idx) => PROCESS_TABLE[idx].read_abi(),
        None => Abi::Lp64,
    }
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

// ─── Deferred EL0 upcall (cooperative-kernel building block 1) ──────
//
// The `request_terminate`/`take_pending_terminate` analogue for the generic
// EL0→EL1 upcall primitive (design §7.1), but per-slot: a consumer posts a
// handler + arguments to a target process's slot, and the SVC handler
// delivers it at that process's next syscall return to EL0. Lock-free over
// the slot atomics; `upcall_pending_handler` (VA, `0` = none) is the
// presence sentinel, written last on post and swapped to `0` on take so a
// reader that observes it set also observes the args/mask stored before it.

/// Post an [`UpcallDesc`] to `slot`, to be delivered at that process's next
/// syscall return to EL0 (design §7.1). A previously-posted, not-yet-
/// delivered upcall is overwritten (last writer wins); the current consumers
/// post at most one pending upcall per delivery point. Out-of-range `slot`
/// is a no-op.
pub fn post_upcall(slot: usize, desc: UpcallDesc) {
    if slot >= MAX_PROCESSES {
        return;
    }
    let p = &PROCESS_TABLE[slot];
    for (i, a) in p.upcall_pending_args.iter().enumerate() {
        a.store(desc.args[i], Ordering::Release);
    }
    p.upcall_pending_mask.store(desc.mask_bit, Ordering::Release);
    p.upcall_pending_disp
        .store(desc.disposition.encode(), Ordering::Release);
    // Presence sentinel last (release): a `take` that sees it nonzero also
    // sees the args/mask/disposition stored above.
    p.upcall_pending_handler
        .store(desc.handler_va, Ordering::Release);
}

/// Consume a pending upcall for `slot`, if any (design §7.1). Swaps the
/// `handler_va` sentinel to `0` and, if it was set, reads back the
/// args/mask posted before it. Out-of-range `slot` yields `None`.
pub fn take_pending_upcall(slot: usize) -> Option<UpcallDesc> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    let p = &PROCESS_TABLE[slot];
    let handler_va = p.upcall_pending_handler.swap(0, Ordering::AcqRel);
    if handler_va == 0 {
        return None;
    }
    let mut args = [0u64; 4];
    for (i, a) in p.upcall_pending_args.iter().enumerate() {
        args[i] = a.load(Ordering::Acquire);
    }
    let mask_bit = p.upcall_pending_mask.load(Ordering::Acquire);
    let disposition =
        Disposition::decode(p.upcall_pending_disp.load(Ordering::Acquire));
    Some(UpcallDesc {
        handler_va,
        args,
        mask_bit,
        disposition,
    })
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
    p.abi.store(Abi::Lp64 as u8, Ordering::Release);
    p.basepage_kva.store(ptr::null_mut(), Ordering::Release);
    p.basepage_pa.store(0, Ordering::Release);
    p.l0_pa.store(0, Ordering::Release);
    p.entry_va.store(0, Ordering::Release);
    p.parent_slot.store(NO_PARENT, Ordering::Release);
    p.parent_frame.clear();
    p.parent_l0_pa.store(0, Ordering::Release);
    // Clear any stale WAITING key so a reused slot does not get spuriously
    // woken by a `sched::wake` matching a dead process's wait condition.
    p.wait_cond.store(0, Ordering::Release);
    // Reset the EL0 upcall state so a reused slot starts clean.
    p.upcall_depth.store(0, Ordering::Release);
    p.upcall_mask.store(0, Ordering::Release);
    p.upcall_pending_handler.store(0, Ordering::Release);
    for a in p.upcall_pending_args.iter() {
        a.store(0, Ordering::Release);
    }
    p.upcall_pending_mask.store(0, Ordering::Release);
    p.upcall_pending_disp.store(0, Ordering::Release);
    for c in p.upcall_ctx.iter() {
        c.clear();
    }
    for m in p.upcall_ctx_mask.iter() {
        m.store(0, Ordering::Release);
    }
    for d in p.upcall_ctx_disp.iter() {
        d.store(0, Ordering::Release);
    }
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
