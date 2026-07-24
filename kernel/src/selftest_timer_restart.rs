//! Cooperative-kernel step-4-prep — timer-deadline S2-restart idempotency test.
//!
//! Spec: `docs/analysis/coop-kernel/20-sleepwake-substrate-design.md` §4.3 OPEN.
//! A timer-bearing wait (`evnt_timer`/`wait_ms`/`evnt_multi`-`MU_TIMER`) computes
//! its deadline as `monotonic_millis() + ms`. Under the S2 restartable block the
//! whole syscall re-executes from the top on resume, so recomputing the deadline
//! at the function head would push it forward by `ms` on every restart — with a
//! live peer it would never converge. This test proves the per-slot
//! [`tos_api::bdos::process::Process::syscall_deadline`] persistence added to
//! [`sched::arm_or_get_deadline`] / [`sched::clear_deadline`]:
//!
//!   * the **first** entry of a timer wait computes + stores `now + ms`;
//!   * an **S2 restart** (a peer ran, time advanced) REUSES the stored deadline —
//!     it is NOT pushed forward by the elapsed time (the empirical proof: the
//!     timer still fires at the ORIGINAL deadline);
//!   * **completion** clears the field, so the next timer wait recomputes fresh;
//!   * the **no-slot** (EL1 desktop, `current() == NONE`) path never persists and
//!     recomputes each call — unchanged from today's behavior.
//!
//! Deterministic: it feeds synthetic `now` values straight into the scheduler
//! primitives (an S2 restart at now=T just calls `arm_or_get_deadline(T, ms)`
//! again), so it needs no real elapsed time, EL0 excursion, or peer switch to
//! exercise the exact first-entry-vs-restart-vs-clear logic. Gated behind
//! `pub mod`, **not** wired into boot (mirrors [`crate::selftest_upcall`]).

use core::sync::atomic::Ordering;

use tos_api::bdos::process::{free_slot, sched, State, MAX_PROCESSES, PROCESS_TABLE};

use crate::kprintln;

/// Slot this test claims as the "running" scheduler slot so the per-slot
/// deadline persistence acts on it. Slot 5 — clear of EC4/D3/D6/D7/sw3b.
const T_SLOT: usize = 5;

/// Milliseconds requested by the modelled timer wait.
const MS: u64 = 500;

/// Run the timer-deadline S2-restart idempotency self-test.
pub fn run() {
    kprintln!("");
    kprintln!("[tmr] === timer-deadline S2-restart idempotency test ===");

    // Make T_SLOT the running scheduler slot (arm/clear act on `current()`),
    // starting from a cleared deadline field.
    PROCESS_TABLE[T_SLOT].set_state(State::Running);
    PROCESS_TABLE[T_SLOT]
        .syscall_deadline
        .store(0, Ordering::Release);
    sched::set_running(T_SLOT);

    // 1. First entry at now = 1000 → deadline = 1500, stored in the slot.
    let d0 = sched::arm_or_get_deadline(1000, MS);
    let stored0 = PROCESS_TABLE[T_SLOT]
        .syscall_deadline
        .load(Ordering::Acquire);

    // 2. S2 restart at now = 1200 (200 ms elapsed while a peer ran and A's `svc`
    //    re-executed): must REUSE 1500, NOT recompute 1200 + 500 = 1700.
    let d1 = sched::arm_or_get_deadline(1200, MS);

    // 3. A further restart at now = 1490 (just before the deadline): still 1500,
    //    NOT 1490 + 500 = 1990 — the deadline never creeps forward.
    let d2 = sched::arm_or_get_deadline(1490, MS);

    // 4. Completion (the timer fired and the wait returned) clears the field.
    sched::clear_deadline();
    let stored_after_clear = PROCESS_TABLE[T_SLOT]
        .syscall_deadline
        .load(Ordering::Acquire);

    // 5. A later, unrelated timer wait recomputes fresh from the new `now`.
    let d3 = sched::arm_or_get_deadline(3000, MS);

    // 6. No-slot (EL1 desktop) path: `current() == NONE` → nothing persisted, so
    //    each call recomputes `now + ms` (exactly today's behavior, no restart).
    sched::set_running(MAX_PROCESSES);
    let dn0 = sched::arm_or_get_deadline(1000, MS);
    let dn1 = sched::arm_or_get_deadline(1200, MS);

    // Verdict.
    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[tmr]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };
    check(d0 == 1500, "(1) first entry computes now+ms (1500)");
    check(stored0 == 1500, "(1) deadline persisted in the slot");
    check(d1 == 1500, "(2) S2 restart REUSES the deadline (not 1700)");
    check(d2 == 1500, "(2) further restart still reuses (not 1990)");
    check(stored_after_clear == 0, "(3) completion clears the field");
    check(d3 == 3500, "(3) next timer wait recomputes fresh (3500)");
    check(
        dn0 == 1500 && dn1 == 1700,
        "    no-slot (desktop) path recomputes each call",
    );

    // Cleanup: clear the field, reset the slot, and drop the running pointer.
    PROCESS_TABLE[T_SLOT]
        .syscall_deadline
        .store(0, Ordering::Release);
    PROCESS_TABLE[T_SLOT].set_state(State::Free);
    free_slot(T_SLOT);
    sched::set_running(MAX_PROCESSES);

    if fails == 0 {
        kprintln!("[tmr] === PASS ===");
    } else {
        kprintln!("[tmr] === FAIL: {} check(s) ===", fails);
    }
}
