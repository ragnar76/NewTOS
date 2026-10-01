//! SCHED01 — **`dsptch` puts the caller on the ready list; it does not assume
//! it is there.**
//!
//! `disp_act` is `p->p_stat &= ~WAITIN; insert_process(p, &rlr);`
//! (`aes/gemdisp.c:74-79`) — an *insert*. EmuTOS's running process is on `rlr`
//! by construction: `disp` takes it off the head (`:225-226`) and `disp_act`
//! links it back in, and a launched program needs no separate admission step
//! because `sh_ldapp` runs it under the **shell's own** AESPD (it renames
//! `rlr` and `dos_exec`s it, `aes/gemshlib.c:604-605,634-636`).
//!
//! NewTOS gives a launched program its own scheduler slot, and nothing admits
//! it: `sched::admit` is called for desk accessories only. The bit was
//! acquired as a *side effect* of `sleep`'s idle exit — "WAITING → READY" —
//! so every program that had blocked in an AES wait at least once was in the
//! run-set, and a program that never blocks was not. Measured
//! (`measurements/measurement-2026-09-29-port08`), one bit apart at the instant of
//! `appl_exit` with two accessories loaded:
//!
//! ```text
//! ball demo (no event loop)  s2(st=2 run=0 ctx=0 cond=0x0)  appl_exit -> never returns
//! TeraDesk  (event loop)     s2(st=2 run=1 ctx=0 cond=0x0)  appl_exit -> returns, Pterms
//! ```
//!
//! With `run=0` the barrier handed the CPU to the accessories and `pick_next`
//! had no candidate to hand it back to. What is asserted here is the `run=0`
//! case — the one the ACC10 test's fixture does not cover, because it seeds
//! the waiter READY — plus the property the fix must not break: a parent
//! suspended inside `Pexec` stays out of the run-set (PORT05/ACC10).
//!
//! One `#[test]` function: the process table is global and `cargo test` runs a
//! file's tests in parallel threads.

use core::sync::atomic::Ordering;

use tos_api::bdos::process::{sched, set_current, State, NO_PARENT, PROCESS_TABLE};
use tos_types::BasePage;

/// The application inside `appl_exit` — a full-screen program with no event
/// loop, so it has never been through a blocking AES wait.
const APP: usize = 2;
/// A desk accessory, woken by the `AC_CLOSE` `mn_cleanup` just posted.
const ACC: usize = 0;
/// A second accessory — the measured card carries two.
const ACC2: usize = 1;

/// Make `slot` the process `running_slot()` reports: `slot_for` matches the
/// BDOS running-basepage `RUN` against a non-`Free` slot's `basepage_kva`.
/// The pointer is an identity, never dereferenced by the code under test.
fn make_running(slot: usize) {
    let fake = (0x1000_0000usize + slot * 0x100) as *mut BasePage;
    PROCESS_TABLE[slot].set_state(State::Running);
    PROCESS_TABLE[slot].basepage_kva.store(fake, Ordering::Release);
    set_current(fake);
}

/// The measured state at `267 >> AES op=19 appl_exit`: the application on the
/// CPU and **not** in the run-set, both accessories awake with their
/// `AC_CLOSE` pending.
fn fixture() {
    for p in PROCESS_TABLE.iter() {
        p.runnable.store(false, Ordering::Release);
        p.has_ctx.store(false, Ordering::Release);
        p.wait_cond.store(0, Ordering::Release);
        p.parent_slot.store(NO_PARENT, Ordering::Release);
        p.parent_runnable.store(false, Ordering::Release);
        p.set_state(State::Free);
        p.basepage_kva
            .store(core::ptr::null_mut(), Ordering::Release);
    }
    sched::set_kernel_parked(false);
    for acc in [ACC, ACC2] {
        PROCESS_TABLE[acc].set_state(State::Running);
        sched::admit(acc);
    }
    // `Process::empty()`'s `runnable == false`, never touched: the demo makes
    // no `evnt_*`, `form_do` or `form_alert` call at all.
    make_running(APP);
}

#[test]
fn dsptch_admits_a_caller_that_never_blocked() {
    fixture();
    assert!(
        !PROCESS_TABLE[APP].runnable.load(Ordering::Acquire),
        "fixture: the demo reaches the barrier outside the run-set"
    );

    // `wait_for_accs` → `all_run()` → `dsptch()`: the first accessory runs.
    let sw = match sched::dsptch() {
        sched::Dispatch::Yield(sw) => sw,
        sched::Dispatch::Idle => panic!("a runnable peer exists — must dispatch"),
    };
    assert_eq!(sw.slot, ACC, "the barrier dispatches to the first accessory");

    // `disp_act(p)` = `insert_process(p, &rlr)`, and that is an insert.
    assert!(
        PROCESS_TABLE[APP].runnable.load(Ordering::Acquire),
        "dsptch must PUT the caller on the ready list, not presuppose it"
    );
    assert_eq!(
        PROCESS_TABLE[APP].wait_cond.load(Ordering::Acquire),
        0,
        "dsptch must not arm a wake key — there is no event to wait for"
    );

    // Therefore the CPU comes back when the accessories block again. This is
    // the step that was missing: with the caller outside the run-set,
    // `pick_next` found nothing and `appl_exit` never returned.
    for acc in [ACC, ACC2] {
        PROCESS_TABLE[acc].runnable.store(false, Ordering::Release);
        PROCESS_TABLE[acc]
            .wait_cond
            .store(sched::wait_mesg(acc), Ordering::Release);
    }
    let back = sched::on_yield(ACC2).expect("the caller is a peer now");
    assert_eq!(
        back.slot, APP,
        "the blocked accessory hands the CPU back to the application in appl_exit"
    );

    // With nothing else runnable the pass is the idle pass — and `disp_act`
    // still ran, because EmuTOS puts the process back on a ready list it is
    // alone on just the same.
    fixture();
    for acc in [ACC, ACC2] {
        PROCESS_TABLE[acc].runnable.store(false, Ordering::Release);
    }
    assert!(
        matches!(sched::dsptch(), sched::Dispatch::Idle),
        "nothing runnable: the pass is the idle pass, not a switch"
    );
    assert!(
        PROCESS_TABLE[APP].runnable.load(Ordering::Acquire),
        "the caller is on the ready list either way"
    );

    // The property the fix must not break (PORT05/ACC10): `Pexec` is
    // synchronous, so a parent suspended for its child must NOT come back into
    // the run-set. `dsptch` touches the **running** slot only, and while the
    // child runs that is the child — `set_current(child_basepage)` follows
    // `suspend_parent_for_child` in the launch path.
    fixture();
    const PARENT: usize = APP;
    const CHILD: usize = 3;
    PROCESS_TABLE[PARENT].runnable.store(true, Ordering::Release);
    PROCESS_TABLE[CHILD].set_state(State::Running);
    PROCESS_TABLE[CHILD]
        .parent_slot
        .store(PARENT as u32, Ordering::Release);
    sched::suspend_parent_for_child(CHILD);
    make_running(CHILD);
    assert!(
        !PROCESS_TABLE[PARENT].runnable.load(Ordering::Acquire),
        "fixture: the parent is withdrawn for as long as the child lives"
    );

    let sw = match sched::dsptch() {
        sched::Dispatch::Yield(sw) => sw,
        sched::Dispatch::Idle => panic!("the accessories are runnable"),
    };
    assert_eq!(sw.slot, ACC, "the child's barrier dispatches to an accessory");
    assert!(
        PROCESS_TABLE[CHILD].runnable.load(Ordering::Acquire),
        "the child — the running slot — is the one that gets admitted"
    );
    assert!(
        !PROCESS_TABLE[PARENT].runnable.load(Ordering::Acquire),
        "and the suspended parent stays asleep: dsptch admits the caller, nobody else"
    );
    // `resume_parent_of_child` still restores exactly what was saved.
    sched::resume_parent_of_child(CHILD);
    assert!(
        PROCESS_TABLE[PARENT].runnable.load(Ordering::Acquire),
        "the parent comes back as it was found"
    );

    set_current(core::ptr::null_mut());
}
