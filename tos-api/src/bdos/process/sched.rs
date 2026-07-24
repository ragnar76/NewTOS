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
        // Step 3a: every scheduler peer is an EL0 process. EL1-internal-thread
        // classification lands in step 4 (design §2/§7.2).
        el1_internal: false,
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

// ─── sleep / wake (sleep/wake substrate, step 3a) ───────────────────
//
// The block primitive `sleep(cond)` replaces the six per-`evnt_*` busy-poll
// loops in `aes/event.rs` (design §5); `wake(cond)` is the FreeMiNT `do_wake`
// analogue that re-arms matching waiters. **Step 3a implements only the
// idle-fallback ("Ausgang a", the no-peer path, §4.4):** with exactly one
// runnable context there is no peer to switch to, so `sleep` degrades to
// exactly today's pump-and-recheck spin. The real switch to a peer
// ("Ausgang b"), the S2 restartable-block (ELR-rewind) and deliver-on-resume
// are **step 3b** and are deliberately absent here — `apply_switch` and the
// trap path are untouched.

/// Block the current context on wake-class key `cond` until its predicate
/// fires, running the cooperative idle in the meantime (design §4.2/§4.4).
///
/// **Idle-fallback only (step 3a).** One cooperative pass: mark the caller
/// WAITING(`cond`), pump host input, run the idle wake-pass ([`idle_wake_pass`],
/// design §5.2), then return READY so the caller re-checks its full predicate
/// and re-sleeps if the event has not fired (the FreeMiNT "wake → re-check →
/// re-sleep" loop, §4.1). This is behaviorally identical to the former
/// per-`evnt_*` `host().pump_input()` spin — the caller's predicate loop is
/// unchanged; only the wait *mechanism* moves here and the WAITING state +
/// `wake` wiring are recorded (so step 4 needs no `event.rs` change, §1.3).
///
/// A no-slot caller — the EL1 desktop, whose `evnt_multi` runs with
/// [`current`]` == NONE` (it is not a scheduler peer, design §1.2/§8.1.8) —
/// skips the slot bookkeeping; its wait is driven purely by its own re-check
/// loop, exactly as today.
///
/// `cond` is built with [`wait_input`]/[`wait_mesg`]/[`wait_timer`].
pub fn sleep(cond: u64) {
    let cur = current();
    if cur < MAX_PROCESSES {
        // RUNNING → WAITING(cond): record the wake key and drop out of the
        // round-robin run-set (§3.1).
        PROCESS_TABLE[cur].wait_cond.store(cond, Ordering::Release);
        PROCESS_TABLE[cur].runnable.store(false, Ordering::Release);
    }

    // Ausgang (a) — the no-peer idle-fallback (§4.4): EmuTOS `schedule()`-idle.
    // Pump input, then run the wake sources. The peer-switch path (Ausgang b)
    // is step 3b, so `on_yield`/`apply_switch` are intentionally not consulted
    // here: with one runnable context there is no peer, and not switching is
    // exactly today's behavior.
    if let Some(h) = host() {
        h.pump_input();
    }
    idle_wake_pass();

    if cur < MAX_PROCESSES {
        // WAITING → READY: the caller now re-evaluates its full predicate.
        PROCESS_TABLE[cur].wait_cond.store(0, Ordering::Release);
        PROCESS_TABLE[cur].runnable.store(true, Ordering::Release);
    }
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
