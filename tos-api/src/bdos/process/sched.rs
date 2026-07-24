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

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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
/// `WAIT_MESG(pid)` class tag: a message in `pid`'s per-app FIFO (§4.1). Woken
/// by [`crate::aes::pd::post_msg`] (design §5.2).
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

/// The `WAIT_MESG(pid)` wake key for message FIFO `pid` (design §4.1). Used by
/// `evnt_mesag`; the poster ([`crate::aes::pd::post_msg`]) wakes on the same key.
#[inline]
pub const fn wait_mesg(pid: i16) -> u64 {
    (CLASS_MESG << WAIT_CLASS_SHIFT) | (pid as u16 as u64)
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
/// A no-slot caller — the EL1 desktop, whose `evnt_multi` runs with
/// [`current`]` == NONE` (it is not a scheduler peer, design §1.2/§8.1.8) — has
/// no peer and no slot bookkeeping, so it always takes Ausgang (a), exactly as
/// today.
///
/// `cond` is built with [`wait_input`]/[`wait_mesg`]/[`wait_timer`].
pub fn sleep(cond: u64) -> SleepOutcome {
    let cur = current();
    if cur < MAX_PROCESSES {
        // RUNNING → WAITING(cond): record the wake key and drop out of the
        // round-robin run-set (§3.1). Done before `on_yield` so `pick_next`
        // skips this slot — a returned `Switch` is therefore always a real peer.
        PROCESS_TABLE[cur].wait_cond.store(cond, Ordering::Release);
        PROCESS_TABLE[cur].runnable.store(false, Ordering::Release);

        // Ausgang (b) — a runnable peer exists (§4.4): yield to it. The handler
        // performs the S2 frame-save (ELR rewound to the svc) + switch; we do
        // NOT touch the trap frame here — `sleep` is deep in a non-leaf syscall
        // and does not own it. In the step-3 single-context production path
        // there is no peer, so this never fires and Ausgang (a) is taken.
        if let Some(sw) = on_yield(cur) {
            return SleepOutcome::Yield(sw);
        }
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

/// The cooperative idle wake-pass (design §5.2): the single point where the
/// input and timer wake sources are polled and turned into WAITING → READY
/// transitions — EmuTOS's `schedule()`/`chkkbd` idle, **not** the ISR (the IRQ
/// handler only fills the ring, design §1.2). Runs once per [`sleep`] pass.
fn idle_wake_pass() {
    let Some(h) = host() else {
        return;
    };
    // WAIT_INPUT: console/keyboard input has landed in the ring → wake input
    // waiters (§5.2, EmuTOS `chkkbd`). Mouse/button changes reach the VDI state
    // through `pump_input` directly and are caught by the waiter's own
    // predicate re-check, so they need no separate wake here.
    if h.console_has_input() {
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
