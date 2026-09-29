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

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use super::{MAX_PROCESSES, PROCESS_TABLE};
use crate::host::host;

/// Sentinel for "no slot" (the kernel/boot is on the CPU, no peer runs).
const NONE: usize = MAX_PROCESSES;

// ─── Wait-condition keys (sleep/wake substrate, step 3a) ────────────
//
// A `wait_cond` is a **wake-class key**, not a per-waiter predicate (design
// §4.1): `wake(cond)` re-arms every waiter whose key matches, and each woken
// waiter then re-checks its *full* predicate (the FreeMiNT `sleep`/`do_wake`
// model, `references/freemint/sys/proc.c:490-716`). The key is a `u64` with the
// class in the high byte and the argument (pid / deadline) in the low 56 bits
// (§8.1.3). `0` is reserved for "not waiting", so no valid key is ever `0`.

/// Bit position of the wake-class tag within a `wait_cond` key.
const WAIT_CLASS_SHIFT: u32 = 56;
/// Mask for the argument (pid / deadline) carried in the low bits of a key.
const WAIT_ARG_MASK: u64 = (1u64 << WAIT_CLASS_SHIFT) - 1;

/// `WAIT_INPUT` class tag: keyboard / button / mouse activity (§4.1). Woken by
/// the cooperative idle wake-pass, never the ISR (design §5.2).
const CLASS_INPUT: u64 = 1;
/// `WAIT_MESG(idx)` class tag: a message in AES identity `idx`'s per-app FIFO
/// (§4.1). Woken by [`crate::aes::pd::post_msg`] (design §5.2).
const CLASS_MESG: u64 = 2;
/// `WAIT_TIMER(deadline)` class tag: `monotonic_millis >= deadline` (§4.1).
/// Woken by the idle wake-pass deadline check (timer-IRQ deferred, design §5.2).
const CLASS_TIMER: u64 = 3;

/// The `WAIT_INPUT` wake key (design §4.1). Used by `evnt_keybd`/`evnt_button`/
/// `evnt_mouse`/`evnt_multi`, which all wait on host input.
#[inline]
pub const fn wait_input() -> u64 {
    CLASS_INPUT << WAIT_CLASS_SHIFT
}

/// The `WAIT_MESG(idx)` wake key for the message FIFO of AES identity `idx`
/// (the descriptor index: a GEMDOS slot or `pd::DESKTOP`; design §4.1). Used
/// by `evnt_mesag`; the poster ([`crate::aes::pd::post_msg`]) wakes on the
/// same key.
#[inline]
pub const fn wait_mesg(idx: usize) -> u64 {
    (CLASS_MESG << WAIT_CLASS_SHIFT) | (idx as u64 & WAIT_ARG_MASK)
}

/// The `WAIT_TIMER(deadline)` wake key for an absolute `monotonic_millis`
/// deadline (design §4.1). Used by `evnt_timer`; the idle wake-pass wakes it
/// once the deadline passes.
#[inline]
pub const fn wait_timer(deadline_ms: u64) -> u64 {
    (CLASS_TIMER << WAIT_CLASS_SHIFT) | (deadline_ms & WAIT_ARG_MASK)
}

/// The wake-class tag of a `wait_cond` key.
#[inline]
fn wait_class(cond: u64) -> u64 {
    cond >> WAIT_CLASS_SHIFT
}

/// The argument (pid / deadline) carried by a `wait_cond` key.
#[inline]
fn wait_arg(cond: u64) -> u64 {
    cond & WAIT_ARG_MASK
}

/// The slot currently on the CPU, or [`NONE`].
static CURRENT: AtomicUsize = AtomicUsize::new(NONE);

/// Per-slot restart-kind of the saved context (S2 restartable block, design
/// §4.3): `true` when the slot yielded **mid-syscall** via [`sleep`] with its
/// `saved_frame.elr` rewound to the `svc` (so `apply_switch` must preserve `x0`
/// on resume), `false` for a leaf `Pwait` yield. Written by the exception
/// handler's shared frame-save when a slot yields, read by [`switch_to`] into
/// [`Switch::restartable`]. Parallel to the `runnable`/`has_ctx` bits; a fresh
/// slot is `false` (the value is only consulted on a `!fresh` resume).
static RESUME_RESTARTABLE: [AtomicBool; MAX_PROCESSES] =
    [const { AtomicBool::new(false) }; MAX_PROCESSES];

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
    /// Initial `SP_EL0` for a `fresh` entry — the slot's
    /// [`super::Process::sp_el0`] (the mapped TPA top the loader computed).
    /// `0` for a slot whose image did not come through `pexec::load` (the
    /// embedded selftest blobs), where the handler keeps its own default.
    pub sp_el0: u64,
    /// Privilege class of the peer being switched to (Option B hybrid model,
    /// design §2). `false` = an EL0 user process (its own TTBR0 → `apply_switch`
    /// installs `l0_pa`); `true` = an EL1-internal cooperative thread
    /// (desktop / screen-manager) for which no TTBR0 install must happen.
    ///
    /// **Prepared but unused in step 3a.** Every peer in the step-3 run-set is
    /// an EL0 process, so this is always `false` and `apply_switch` is
    /// unchanged. Step 4 (`docs/analysis/coop-kernel/`
    /// `20-sleepwake-substrate-design.md` §2/§7.2) reads it to skip the TTBR0
    /// switch for EL1-internal threads.
    pub el1_internal: bool,
    /// Restart-kind of the resume (S2 restartable block, design §4.3 + §8.2
    /// sub-step 3b). Only meaningful when `!fresh` (a resume). `false` = the
    /// slot yielded via the leaf `Pwait`: its `saved_frame.elr` points *after*
    /// the `svc`, so `apply_switch` writes `gpr[0] = 0` (the `Pwait` return
    /// value). `true` = the slot yielded mid-syscall via [`sleep`] with its
    /// `saved_frame.elr` rewound to the `svc`: the `eret` **re-executes** that
    /// `svc`, so `apply_switch` must **preserve `x0`** (the GEM-trap selector,
    /// e.g. 200 = AES) — it skips the `gpr[0] = 0` clobber.
    ///
    /// **This is the second S2 delta.** The design §4.3 originally claimed the
    /// only delta vs. the `Pwait` save was the ELR-rewind; that is incomplete —
    /// without preserving `x0` the re-executed `svc #2` re-enters the GEM trap
    /// with `x0 = 0` and dispatches nothing. [`switch_to`] fills this from the
    /// slot's [`set_resume_restartable`] flag, recorded when the slot yielded.
    pub restartable: bool,
}

/// The slot currently running (or [`NONE`]).
#[inline]
pub fn current() -> usize {
    CURRENT.load(Ordering::Acquire)
}

/// The slot of the execution context physically on the CPU, derived from the
/// BDOS running-basepage `RUN` — or [`NONE`] for an **EL1 kernel context**
/// (the boot path, the built-in desktop), which owns no process slot.
///
/// This is the same derivation [`crate::aes::appl::current_identity`] uses and
/// it is the one that is *always* maintained: every path that gives the CPU to
/// an EL0 process sets `RUN` ([`crate::bdos::process::set_current`]) — the
/// kernel launcher, `Pexec`, the scheduler switch — whereas [`CURRENT`] is
/// written only by the scheduler's own [`set_running`] and is therefore
/// [`NONE`] for a program the kernel launcher entered directly.
///
/// [`sleep`] must distinguish "an EL0 process is blocking" from "the EL1
/// kernel client is blocking" (they park in different places, §-ACC03), so it
/// keys off this rather than off [`current`].
#[inline]
pub fn running_slot() -> usize {
    match super::slot_for(super::current()) {
        Some(idx) => idx,
        None => NONE,
    }
}

/// `true` while an **EL1 kernel client** (the boot path's accessory wait, or
/// the built-in desktop inside a blocking `evnt_*`) has its trap frame parked
/// by the kernel and is waiting to be resumed — the NewTOS stand-in for
/// EmuTOS's "the desktop's AESPD is one more entry in the round-robin"
/// (`aes/gemasm.S:169-194` `switchto`, over `rlr`/`drl`).
///
/// EmuTOS has no equivalent flag because it has no privilege boundary: its
/// desktop is an ordinary AESPD and `dsptch()` switches to it like to any
/// other. NewTOS's desktop runs at EL1, owns no process slot and therefore
/// cannot be a [`pick_next`] candidate; instead the trap handler snapshots its
/// frame ("the kernel park") before switching to an accessory, and [`sleep`]
/// consults this flag to know that such a context is waiting to be resumed.
static KERNEL_PARKED: AtomicBool = AtomicBool::new(false);

/// Record whether an EL1 kernel client's frame is parked and resumable — set
/// by the trap handler when it parks, cleared when it restores.
#[inline]
pub fn set_kernel_parked(parked: bool) {
    KERNEL_PARKED.store(parked, Ordering::Release);
}

/// `true` when [`set_kernel_parked`] armed a parked EL1 kernel client.
#[inline]
pub fn kernel_parked() -> bool {
    KERNEL_PARKED.load(Ordering::Acquire)
}

/// The [`Switch`] that means "resume the parked EL1 kernel client" rather than
/// an EL0 peer — `el1_internal` is the flag reserved for exactly this
/// (design §2/§7.2) and `slot` is [`NONE`], so no process-table entry is read.
/// `apply_switch` restores the kernel park instead of a slot's saved frame.
#[inline]
pub fn switch_to_kernel() -> Switch {
    Switch {
        slot: NONE,
        fresh: false,
        l0_pa: 0,
        entry_va: 0,
        sp_el0: 0,
        el1_internal: true,
        restartable: false,
    }
}

/// The next runnable EL0 peer for a context that owns no slot (the EL1 kernel
/// client). `None` when nothing is runnable, i.e. the kernel client has no
/// accessory to give the CPU to and must fall back to the idle pass.
///
/// Scans from slot **0** rather than wrapping like [`pick_next`], so a kernel
/// client that dispatches several fresh peers runs them in slot order — which
/// for desk accessories is discovery order, the order EmuTOS starts them in
/// (`geminit.c:309-315` loads `acc[0..n]` in turn and `pstart` gives them
/// ascending pids) and therefore the order their `menu_register` calls fall in
/// under *Desk*.
pub fn pick_peer_for_kernel() -> Option<Switch> {
    (0..MAX_PROCESSES)
        .find(|i| PROCESS_TABLE[*i].runnable.load(Ordering::Acquire))
        .map(switch_to)
}

/// Record `slot` as the running process.
#[inline]
pub fn set_running(slot: usize) {
    CURRENT.store(slot, Ordering::Release);
}

/// A launch-mode `Pexec` (`PE_LOADGO` / `PE_GO`) has given the CPU to the
/// process in `child_slot`: take its **parent** out of the cooperative run-set
/// for as long as the child lives.
///
/// GEMDOS `Pexec` is a *synchronous* BDOS call. EmuTOS's `proc_go` sets
/// `run = p` and enters the child through `gouser()`
/// (`references/emutos/bdos/proc.c:536-577`); the parent's registers stay
/// frozen in its PD until the child's `xterm` does `run = run->p_parent` and
/// `termuser()` (`:601-618`). Nothing can schedule the parent in between —
/// EmuTOS's AES round-robin cannot even see it, because the launching
/// application and the launched one share **one** AESPD, so `dsptch()` has no
/// second entry to pick (`docs/analysis/coop-kernel/emutos-cooperative-model.md`
/// §5.2: *"GEMDOS `Pexec` … hat keine Verbindung zum AES-`dsptch()`-Scheduler.
/// Während die Kindanwendung läuft, laufen Accessories/Screen-Manager
/// nicht"*).
///
/// NewTOS gives parent and child **separate** slots, and a parent that ever
/// blocked in an `evnt_*` wait is left `runnable` by [`sleep`]'s idle exit — so
/// without this the child's first block would hand the CPU straight back to its
/// own suspended parent (measured: `measurement-2026-09-25-port05`, the parent
/// was re-entered `fresh` at its entry point and relaunched the child in a
/// loop). Dropping the bit is what expresses "the parent is inside `Pexec`".
///
/// Accessories keep running while the child is blocked exactly as before: they
/// are separate run-set members, and only the one parent is withdrawn.
///
/// No-op for a child whose `parent_slot` is [`super::NO_PARENT`] — the
/// kernel-launched root program (`kernel/src/launch.rs`) and desk accessories,
/// whose "parent" is an EL1 context that owns no slot and is not in the
/// run-set to begin with.
pub fn suspend_parent_for_child(child_slot: usize) {
    if child_slot >= MAX_PROCESSES {
        return;
    }
    let parent = PROCESS_TABLE[child_slot].parent_slot.load(Ordering::Acquire);
    if parent == super::NO_PARENT {
        return;
    }
    let p = &PROCESS_TABLE[parent as usize];
    PROCESS_TABLE[child_slot]
        .parent_runnable
        .store(p.runnable.load(Ordering::Acquire), Ordering::Release);
    p.runnable.store(false, Ordering::Release);
    // A process that reached `Pexec` is RUNNING, so by the WAITING/READY rule
    // (§3.2) its `wait_cond` is already `0`. Store it anyway: a stale wake key
    // would let [`wake`] re-arm `runnable` behind our back and put the
    // suspended parent back into `pick_next` while its child is still alive —
    // the exact hole this function closes.
    p.wait_cond.store(0, Ordering::Release);
}

/// The child in `child_slot` has terminated: put its parent back into the
/// cooperative run-set exactly as [`suspend_parent_for_child`] found it.
///
/// Called from the `Pterm` tail **before** the slot is released (releasing it
/// clears `parent_slot` and `parent_runnable`). Restores the saved bit rather
/// than forcing `true`, so a parent that was never a run-set member — a
/// program the kernel launcher started that has not yet blocked in an `evnt_*`
/// wait — does not become one by having launched a child.
pub fn resume_parent_of_child(child_slot: usize) {
    if child_slot >= MAX_PROCESSES {
        return;
    }
    let parent = PROCESS_TABLE[child_slot].parent_slot.load(Ordering::Acquire);
    if parent == super::NO_PARENT {
        return;
    }
    let was = PROCESS_TABLE[child_slot]
        .parent_runnable
        .load(Ordering::Acquire);
    PROCESS_TABLE[parent as usize]
        .runnable
        .store(was, Ordering::Release);
}

/// Add `slot` to the cooperative run-set as a fresh (never-entered) peer.
pub fn admit(slot: usize) {
    if slot < MAX_PROCESSES {
        PROCESS_TABLE[slot].has_ctx.store(false, Ordering::Release);
        PROCESS_TABLE[slot].runnable.store(true, Ordering::Release);
        // A fresh peer has no saved frame; clear any stale restart-kind left by
        // a previous tenant of this slot so a later resume can't misread it.
        RESUME_RESTARTABLE[slot].store(false, Ordering::Release);
    }
}

/// Record how slot `slot` yielded, for the resume side (S2 restartable block,
/// design §4.3): `restartable = true` for a mid-syscall [`sleep`] yield (the
/// `saved_frame.elr` is rewound to the `svc`, so the resume preserves `x0`),
/// `false` for a leaf `Pwait` yield. Called by the exception handler's shared
/// frame-save; read by [`switch_to`] into [`Switch::restartable`].
#[inline]
pub fn set_resume_restartable(slot: usize, restartable: bool) {
    if slot < MAX_PROCESSES {
        RESUME_RESTARTABLE[slot].store(restartable, Ordering::Release);
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
        sp_el0: p.sp_el0.load(Ordering::Acquire),
        // Step 3a: every scheduler peer is an EL0 process. EL1-internal-thread
        // classification lands in step 4 (design §2/§7.2).
        el1_internal: false,
        // How the slot last yielded — consulted only when `!fresh` to decide
        // whether the resume preserves `x0` (S2) or writes the `Pwait` 0.
        restartable: RESUME_RESTARTABLE[slot].load(Ordering::Acquire),
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

// ─── sleep / wake (sleep/wake substrate, steps 3a + 3b) ─────────────
//
// The block primitive `sleep(cond)` replaces the six per-`evnt_*` busy-poll
// loops in `aes/event.rs` (design §5); `wake(cond)` is the FreeMiNT `do_wake`
// analogue that re-arms matching waiters. `sleep` has the two exits of design
// §4.4: **Ausgang (a)**, the no-peer idle-fallback (step 3a) — with exactly one
// runnable context there is no peer to switch to, so `sleep` degrades to
// exactly today's pump-and-recheck spin — and **Ausgang (b)**, the real peer
// switch (step 3b): when a runnable peer exists, `sleep` returns
// [`SleepOutcome::Yield`] so the caller aborts its (non-leaf) syscall and the
// trap handler performs the S2 restartable-block save + `apply_switch`.

/// The outcome of a [`sleep`] call — its two exits (design §4.4).
#[must_use]
pub enum SleepOutcome {
    /// **Ausgang (a).** The idle-fallback ran (no runnable peer — the step-3
    /// single-context case): host input was pumped and the wake sources polled.
    /// The caller re-checks its full predicate and re-sleeps if the event has
    /// not fired.
    Recheck,
    /// **Ausgang (b).** A runnable peer exists (step 3b): the caller must
    /// **abort its syscall** and propagate this `Switch` up to the trap handler,
    /// which saves the caller's frame with the ELR rewound to the `svc` (S2
    /// restartable block, §4.3) and switches to the peer. The caller must **not**
    /// commit any results — the syscall re-executes from the top on resume.
    Yield(Switch),
}

/// Block the current context on wake-class key `cond` until its predicate
/// fires (design §4.2/§4.4). Returns one of the two [`SleepOutcome`] exits.
///
/// Mark the caller WAITING(`cond`) (drop out of the round-robin run-set, §3.1),
/// then:
///
///   * **Ausgang (b)** — if a runnable **peer** exists, return
///     [`SleepOutcome::Yield`] carrying the [`Switch`] to it. `sleep` sits deep
///     inside a non-leaf syscall (`evnt_multi`) and does **not** own the trap
///     frame, so it cannot switch here: the caller propagates the `Yield` up to
///     `rust_sync_handler`, which does the S2 frame-save (ELR rewound to the
///     `svc`) + `apply_switch`. The caller left the slot WAITING; on resume the
///     re-executed `svc` re-enters the syscall, re-checks, and fires.
///   * **Ausgang (a)** — otherwise (no peer, the step-3 single-context case):
///     pump host input, run the idle wake-pass ([`idle_wake_pass`], §5.2), mark
///     the caller READY again, and return [`SleepOutcome::Recheck`]. This is
///     behaviorally identical to the former per-`evnt_*` `host().pump_input()`
///     spin — only the wait *mechanism* moves here.
///
/// **The EL1 kernel client (ACC03).** A caller that owns no slot — the boot
/// path and the built-in desktop, both of which reach `evnt_*` from EL1 — is
/// not a [`pick_next`] candidate and has nothing to mark WAITING. It still has
/// the two exits, mirrored:
///
///   * **Ausgang (b)** — a runnable EL0 peer exists (a desk accessory that was
///     woken by [`crate::aes::pd::post_msg`]): yield to it. The handler parks
///     the EL1 frame (the "kernel park") and switches; when the accessory
///     blocks again it comes straight back here and the rewound `svc`
///     re-enters this wait. This is EmuTOS's `dsptch()` from inside `ev_block`
///     (`aes/gemevlib.c`, `gemasm.S:169-194`), for the one client that is not
///     a process.
///   * **Ausgang (a)** — nothing runnable: the idle pass, exactly as before.
///
/// **Yielding *to* the parked kernel client.** The symmetric case: an EL0
/// accessory blocks, no EL0 peer is runnable, and an EL1 client is parked
/// ([`kernel_parked`]). Without this exit the accessory would spin in the idle
/// fallback forever and the desktop — which is not in the run-set and can
/// therefore never be picked — would never run again. `sleep` returns
/// [`switch_to_kernel`], which `apply_switch` turns into the kernel-park
/// restore.
///
/// `cond` is built with [`wait_input`]/[`wait_mesg`]/[`wait_timer`].
pub fn sleep(cond: u64) -> SleepOutcome {
    let cur = running_slot();
    if cur < MAX_PROCESSES {
        // RUNNING → WAITING(cond): record the wake key and drop out of the
        // round-robin run-set (§3.1). Done before `on_yield` so `pick_next`
        // skips this slot — a returned `Switch` is therefore always a real peer.
        PROCESS_TABLE[cur].wait_cond.store(cond, Ordering::Release);
        PROCESS_TABLE[cur].runnable.store(false, Ordering::Release);

        // Ausgang (b) — a runnable peer exists (§4.4): yield to it. The handler
        // performs the S2 frame-save (ELR rewound to the svc) + switch; we do
        // NOT touch the trap frame here — `sleep` is deep in a non-leaf syscall
        // and does not own it.
        if let Some(sw) = on_yield(cur) {
            return SleepOutcome::Yield(sw);
        }
        // No EL0 peer — but an EL1 kernel client may be parked waiting for
        // exactly this moment (ACC03). Give it the CPU back rather than spin.
        if kernel_parked() {
            return SleepOutcome::Yield(switch_to_kernel());
        }
    } else if let Some(sw) = pick_peer_for_kernel() {
        // The EL1 kernel client blocks and an accessory is runnable: `dsptch()`.
        return SleepOutcome::Yield(sw);
    }

    // Ausgang (a) — the no-peer idle-fallback (§4.4): EmuTOS `schedule()`-idle.
    // Pump input, then run the wake sources. Behaviorally identical to today's
    // per-`evnt_*` pump-and-recheck spin.
    if let Some(h) = host() {
        h.pump_input();
    }
    idle_wake_pass();

    if cur < MAX_PROCESSES {
        // WAITING → READY: the caller now re-evaluates its full predicate.
        PROCESS_TABLE[cur].wait_cond.store(0, Ordering::Release);
        PROCESS_TABLE[cur].runnable.store(true, Ordering::Release);
    }
    SleepOutcome::Recheck
}

/// Wake every context WAITING on exactly `cond` (WAITING → READY, design §4.2):
/// clear its [`super::Process::wait_cond`] and re-arm its `runnable` bit. The
/// FreeMiNT `do_wake` analogue (`references/freemint/sys/proc.c:665-701`) — it
/// does **not** switch immediately; a woken peer runs only when the current
/// context next [`sleep`]s or exits (design §4.2).
///
/// In step 3a (one runnable context) `wake` is not yet load-bearing — the
/// caller's own idle re-check already observes the event. It is wired now so
/// the step-4 peer-switch path and the message/input/timer posters do not have
/// to touch `event.rs` later (design §1.3).
pub fn wake(cond: u64) {
    if cond == 0 {
        return; // 0 is "not waiting", never a valid wake key.
    }
    for p in PROCESS_TABLE.iter() {
        if p.wait_cond.load(Ordering::Acquire) == cond {
            p.wait_cond.store(0, Ordering::Release);
            p.runnable.store(true, Ordering::Release);
        }
    }
}

// ─── dsptch: the yield that keeps the caller on the ready list ──────────────
//
// `sleep` above is EmuTOS's `WAITIN` branch. This is the other one, and the AES
// needs both: a poll loop whose condition no wake class covers must yield, not
// sleep, or nothing will ever wake it (ACC10).

/// The outcome of a [`dsptch`] pass — the two exits of [`SleepOutcome`], with
/// the caller left **on** the ready list.
#[must_use]
pub enum Dispatch {
    /// A ready **peer** got the CPU. Like [`SleepOutcome::Yield`] the caller
    /// must abort its syscall and propagate the [`Switch`] to the trap handler
    /// (the S2 restartable block, design §4.3); unlike it, the caller is READY,
    /// so the round-robin comes back on its own.
    Yield(Switch),
    /// Nobody else was ready: the idle pass ran (host input pumped, wake
    /// sources polled) and the caller simply carries on.
    Idle,
}

/// `dsptch()` (`aes/gemasm.S:129-167` → `disp`, `aes/gemdisp.c:220-254`) — give
/// the CPU to the next ready context **without leaving the ready list**.
///
/// The second block primitive next to [`sleep`], and the difference is the
/// entire point. `disp` takes the running process off the head of `rlr` and
/// then branches on its *own* state (`gemdisp.c:229-231`):
///
/// ```text
/// if (p->p_stat & WAITIN)  mwait_act(p);   /* park it on nrl  -> `sleep`  */
/// else                     disp_act(p);    /* back on rlr     -> `dsptch` */
/// ```
///
/// and `disp_act` is `insert_process(p, &rlr)` (`gemdisp.c:74-79`) — straight
/// back onto the ready list. A process that calls `dsptch()` outside an
/// `ev_block` has not set `WAITIN`, so it is rescheduled unconditionally, with
/// no wake source, no event and no input involved.
///
/// **It is an insert, not a promise.** `disp_act` *puts* the caller on the
/// ready list; it does not assume it is there. That distinction is the whole
/// of SCHED01: NewTOS has no `pstart`-equivalent for a launched program — in
/// EmuTOS one is not needed, because `sh_ldapp` runs the program under the
/// *shell's own* AESPD (`aes/gemshlib.c:604-605,634-636`: it renames `rlr`
/// and `dos_exec`s), and `rlr` is by definition the ready list — so the only
/// thing that ever made a NewTOS program a `pick_next` candidate was the
/// `runnable.store(true)` in [`sleep`]'s idle exit. A program that never
/// blocks in an AES wait therefore reached `wait_for_accs` outside the
/// run-set and hung in `appl_exit`. This function establishes the membership
/// that EmuTOS has by construction.
///
/// **When to use which.** [`sleep`] is right for a wait that a wake class
/// covers (`evnt_keybd`, `evnt_mesag`, `evnt_timer`): the caller is parked and
/// the poster wakes it. `dsptch` is right for a poll loop over state no wake
/// class covers — `wait_for_accs`'s walk over the desk accessories' `p_flags`
/// (`aes/geminit.c:609-626`) is the AES's one instance. Sleeping on an
/// unrelated class there parks the caller until something unrelated happens,
/// which on a machine nobody is touching is never (ACC10; the barrier used to
/// sleep on `wait_input()` and a headless `appl_exit` did not return).
///
/// The counterpart for the *other* half of `disp` — the `forker(); schedule();`
/// input drain every completed AES call ends with — is `aes::dsptch`.
pub fn dsptch() -> Dispatch {
    let cur = running_slot();
    if cur < MAX_PROCESSES {
        // `disp_act(p)` = `insert_process(p, &rlr)` (`gemdisp.c:74-79`): the
        // caller is **put on** the ready list. Establishing the membership is
        // the point — EmuTOS's running process is on `rlr` by construction
        // (`disp` takes it off the head at `:225-226`, `disp_act` links it
        // back in at the tail), and a NewTOS slot has no such guarantee:
        // `admit` is called for desk accessories only (`aes/acc.rs`), and a
        // launched program acquires the bit solely as a side effect of
        // [`sleep`]'s idle exit, i.e. only once it has blocked in some AES
        // wait. A full-screen program with no event loop never blocks, so it
        // reached this line with `runnable == false`, handed the CPU to the
        // accessories, and `pick_next` then had no candidate to hand it back
        // to: measured, the ball demo never returned from `appl_exit` with an
        // accessory loaded (SCHED01, `measurement-2026-09-29-port08`). Set
        // before the pick, so the peer we switch to already sees the caller on
        // the list.
        //
        // `wait_cond` is deliberately left alone. EmuTOS's `disp_act` clears
        // `WAITIN` because `mwait_act` calls it too, for a waiter whose event
        // flags are already set (`gemdisp.c:82-90`); our counterpart of that
        // shortcut is [`sleep`]'s idle exit, which clears the key itself. In
        // the `else` branch — the one this function is — the caller is RUNNING
        // and its key is `0` by the WAITING/READY rule (§3.2), so the clear is
        // the no-op it is in EmuTOS.
        PROCESS_TABLE[cur].runnable.store(true, Ordering::Release);
        if let Some(sw) = on_yield(cur) {
            return Dispatch::Yield(sw);
        }
        // No EL0 peer, but an EL1 kernel client may be parked (ACC03) — it is
        // one more entry on EmuTOS's ready list, so it is a `dsptch` target.
        if kernel_parked() {
            return Dispatch::Yield(switch_to_kernel());
        }
    } else if let Some(sw) = pick_peer_for_kernel() {
        // The EL1 kernel client dispatches: an accessory is runnable.
        return Dispatch::Yield(sw);
    }

    // Nothing to switch to. `disp`'s `schedule()` still runs (`gemdisp.c:240`),
    // which is our idle pass; then the caller carries on, exactly as EmuTOS's
    // would after being put straight back on a ready list it is alone on.
    if let Some(h) = host() {
        h.pump_input();
    }
    idle_wake_pass();
    Dispatch::Idle
}

// ─── timer deadline persistence (S2-restart idempotency, §4.3 OPEN) ─────────
//
// A timer-bearing wait (`evnt_timer`/`wait_ms`/`evnt_multi`-`MU_TIMER`) computes
// its deadline as `monotonic_millis() + ms`. Under the S2 restartable block
// (§4.3) the whole syscall re-executes from the top on resume, so recomputing
// the deadline at the function head would push it forward by `ms` on every
// restart — with a live peer it would never converge (irrelevant with one
// context/no peer, real with step-4 peers). Persisting the deadline per-slot in
// [`super::Process::syscall_deadline`] makes a restart REUSE the original
// deadline; first-entry-vs-restart is detected by the field being `0` vs set.

/// Arm (first entry) or reuse (S2 restart) the running slot's timer deadline
/// (design §4.3). When the slot's [`super::Process::syscall_deadline`] is `0`
/// (first entry), computes `now + ms`, stores it, and returns it; when it is
/// already set (an S2 restart re-entered the wait), returns the STORED value
/// unchanged — so the elapsed time while a peer ran does not move the deadline.
/// Cleared on completion by [`clear_deadline`].
///
/// With no running scheduler slot — the EL1 desktop, whose timer waits run with
/// [`current`]` == NONE` and never restart (idle-fallback only, §4.4) — there is
/// nothing to persist, so it just returns `now + ms` (today's behavior).
pub fn arm_or_get_deadline(now: u64, ms: u64) -> u64 {
    let cur = current();
    if cur < MAX_PROCESSES {
        let field = &PROCESS_TABLE[cur].syscall_deadline;
        let stored = field.load(Ordering::Acquire);
        if stored != 0 {
            return stored; // S2 restart: reuse the original deadline
        }
        let deadline = now.saturating_add(ms);
        field.store(deadline, Ordering::Release);
        deadline
    } else {
        now.saturating_add(ms)
    }
}

/// Clear the running slot's persisted timer deadline (design §4.3). Called on
/// EVERY completion path of a timer-bearing wait — it fires and returns, or
/// another event (input/message) fires first in `evnt_multi` — so a later,
/// unrelated timer wait recomputes a fresh deadline via [`arm_or_get_deadline`].
/// A no-op when no slot is running or no deadline was armed.
pub fn clear_deadline() {
    let cur = current();
    if cur < MAX_PROCESSES {
        PROCESS_TABLE[cur].syscall_deadline.store(0, Ordering::Release);
    }
}

/// Pointer state seen by the previous [`idle_wake_pass`] — `x`, `y` and the
/// button mask packed into one word, so one atomic compare detects any
/// mouse-borne event. Seeded to a value no real pointer state can take
/// (`u64::MAX`) so the first pass always reports a change.
static LAST_POINTER: AtomicU64 = AtomicU64::new(u64::MAX);

/// The live pointer state as one word: `x`, `y` and the button mask.
fn mouse_state_word() -> u64 {
    let (mx, my) = crate::vdi::mouse_position();
    ((mx as u16 as u64) << 32) | ((my as u16 as u64) << 16) | (crate::vdi::mouse_buttons() as u64)
}

/// The cooperative idle wake-pass (design §5.2): the single point where the
/// input and timer wake sources are polled and turned into WAITING → READY
/// transitions — EmuTOS's `schedule()`/`chkkbd` idle, **not** the ISR (the IRQ
/// handler only fills the ring, design §1.2). Runs once per [`sleep`] pass.
fn idle_wake_pass() {
    let Some(h) = host() else {
        return;
    };
    // WAIT_INPUT: console/keyboard input has landed in the ring → wake input
    // waiters (§5.2, EmuTOS `chkkbd`).
    //
    // **Mouse/button changes wake them too** (ACC03). The previous rule — "they
    // reach the VDI state through `pump_input` and are caught by the waiter's
    // own predicate re-check" — holds only while the waiter *is* the context
    // running the idle pass. With a second client in the run-set the idle pass
    // runs in whichever context happens to be on the CPU, and a peer WAITING on
    // `MU_BUTTON`/`MU_M1` can only re-check its predicate once it is scheduled
    // again — which needs a wake. `pump_input` above has already folded the
    // ring into the VDI pointer state, so comparing that state against the
    // previous pass is the cheapest faithful "something happened" test; a
    // spurious wake is harmless (the waiter re-checks and re-sleeps), a missing
    // one is a lost event.
    let pointer = mouse_state_word();
    if h.console_has_input() || pointer != LAST_POINTER.swap(pointer, Ordering::AcqRel) {
        wake(wait_input());
    }
    // WAIT_TIMER: per-waiter deadline reached → wake (timer-IRQ deferred to
    // step 4, §5.2). The absolute deadline lives in the low bits of the key.
    let now = h.monotonic_millis();
    for p in PROCESS_TABLE.iter() {
        let c = p.wait_cond.load(Ordering::Acquire);
        if wait_class(c) == CLASS_TIMER && wait_arg(c) <= now {
            p.wait_cond.store(0, Ordering::Release);
            p.runnable.store(true, Ordering::Release);
        }
    }
}
