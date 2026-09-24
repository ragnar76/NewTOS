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
