//! U91-INSTR-TEMP -- DESK43 U9-1 scratch instruments (never in the product; applied to a scratch
//! copy by method/u91-instr.py). docs/analysis/desk43-u9-prep-2026-10-14.md §5.1 (stack) / §5.2
//! (start-up latency).
//!
//! Stack (paint and measure, EmuTOS CONF_DEBUG_DESK_STACK: deskstart.S:43-51 fills, deskmain.c:404-417
//! counts the untouched words from the low end):
//!   * EL1 (built-in desktop): the desktop runs on the main kernel stack (SP_EL0 with SPSel = 0,
//!     [__stack_bottom_inclusive, __stack_top_exclusive), link-*.ld; every exception -- also its own
//!     `svc`s -- runs on SP_EL1). Right before `desktop::run()` in sh_ldapp's DESKTOP arm, the part
//!     [__stack_bottom_inclusive, SP - 1 KiB) is painted; depth = SP_at_call - lowest touched word.
//!   * EL0 (a launched program, NEWDESK): right before the drop to EL0 the launcher's hook paints the
//!     program's crt0 stack, [B + round16(_end - B), +MINKEEP) with _end = p_bbase + p_blen
//!     (sdk/newtos.ld: p_bbase + p_blen == _end; sdk/crt0.S: SP = B + round16(_end - B) + MINKEEP),
//!     through the kernel alias (VA = PA); depth = top - lowest touched word.
//!   Reported at "desktop up", once a second (pump_devices) when the value grew, when the EL1 desktop
//!   returns and when the EL0 child terminates.
//!
//! Latency (CNTVCT_EL0, microseconds; hz_200 alongside):
//!   A = sh_ldapp entry, B = launcher hands over to EL0 (EL0 only), U = desktop up (EL1: the kernel's
//!   "[desk] desktop up"; EL0: the shell child's first evnt_multi after appl_init), X = the shell
//!   child's termination (Pterm, kernel-launch branch of finish_terminate).
//!   Lines: "[u91] lat n=<sh_ldapp count> <kind> A->U=.. us (A->B=.., B->U=..) X->A=.. X->U=.."
#![allow(dead_code, missing_docs, clippy::all)]

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

const MARKER: u64 = 0xDEAD_BEEF_DEAD_BEEF;
const MINKEEP: u64 = 0x10000;
const NONE: u64 = u64::MAX;

extern "C" {
    static __stack_bottom_inclusive: u8;
    static __stack_top_exclusive: u8;
}

fn cntvct() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("isb", "mrs {}, cntvct_el0", out(reg) v, options(nomem, nostack)) };
    v
}
fn cntfrq() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("mrs {}, cntfrq_el0", out(reg) v, options(nomem, nostack)) };
    v
}
fn us(d: u64) -> u64 {
    let f = cntfrq().max(1);
    d.saturating_mul(1_000_000) / f
}
fn sp() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("mov {}, sp", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}
fn hz() -> u32 {
    tos_api::bios::tick::hz_200()
}

// ── latency ──────────────────────────────────────────────────────────────────
static N: AtomicU32 = AtomicU32::new(0);
static T_A: AtomicU64 = AtomicU64::new(NONE);
static T_B: AtomicU64 = AtomicU64::new(NONE);
static T_X: AtomicU64 = AtomicU64::new(NONE);
static UP_REPORTED: AtomicBool = AtomicBool::new(false);

/// sh_ldapp entry (A).
pub fn ldapp(nextapp: impl core::fmt::Display, cmd: &[u8]) {
    let n = N.fetch_add(1, Ordering::Relaxed) + 1;
    let t = cntvct();
    T_A.store(t, Ordering::Relaxed);
    T_B.store(NONE, Ordering::Relaxed);
    UP_REPORTED.store(false, Ordering::Relaxed);
    let x = T_X.load(Ordering::Relaxed);
    let len = cmd.iter().position(|&c| c == 0).unwrap_or(cmd.len());
    crate::kprintln!(
        "[u91] A n={} nextapp={} cmd='{}' hz={} t={} X->A={}",
        n,
        nextapp,
        core::str::from_utf8(&cmd[..len]).unwrap_or("?"),
        hz(),
        t,
        if x == NONE { -1 } else { us(t - x) as i64 }
    );
}

fn up(kind: &str) {
    if UP_REPORTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let t = cntvct();
    let a = T_A.load(Ordering::Relaxed);
    let b = T_B.load(Ordering::Relaxed);
    let x = T_X.load(Ordering::Relaxed);
    let d = |p: u64, q: u64| if p == NONE || q == NONE || q < p { -1 } else { us(q - p) as i64 };
    crate::kprintln!(
        "[u91] lat n={} {} A->U={} us (A->B={} B->U={}) X->U={} hz={}",
        N.load(Ordering::Relaxed),
        kind,
        d(a, t),
        d(a, b),
        d(b, t),
        d(x, t),
        hz()
    );
}

// ── stack, EL1 ───────────────────────────────────────────────────────────────
static EL1_TOP: AtomicU64 = AtomicU64::new(0); // SP at the desktop::run() call; 0 = not painted
static EL1_MAX: AtomicU64 = AtomicU64::new(0);
static EL1_RUNS: AtomicU32 = AtomicU32::new(0);

fn el1_bounds() -> (u64, u64) {
    unsafe {
        (
            core::ptr::addr_of!(__stack_bottom_inclusive) as u64,
            core::ptr::addr_of!(__stack_top_exclusive) as u64,
        )
    }
}

/// Right before `desktop::run()` (DESKTOP arm).
#[inline(never)]
pub fn el1_paint() {
    let s = sp();
    let (lo, _hi) = el1_bounds();
    let end = (s - 1024) & !7;
    let mut p = lo;
    while p < end {
        unsafe { (p as *mut u64).write_volatile(MARKER) };
        p += 8;
    }
    let run = EL1_RUNS.fetch_add(1, Ordering::Relaxed) + 1;
    EL1_TOP.store(s, Ordering::Release);
    EL1_MAX.store(0, Ordering::Relaxed);
    crate::kprintln!("[u91] stk el1 painted run={} sp=0x{:x} bottom=0x{:x} room={} B", run, s, lo, s - lo);
}

fn el1_depth() -> Option<u64> {
    let top = EL1_TOP.load(Ordering::Acquire);
    if top == 0 {
        return None;
    }
    let (lo, _) = el1_bounds();
    let mut p = lo;
    while p < top && unsafe { (p as *const u64).read_volatile() } == MARKER {
        p += 8;
    }
    Some(top - p)
}

fn el1_report(ev: &str, only_if_grown: bool) {
    let Some(d) = el1_depth() else { return };
    let prev = EL1_MAX.fetch_max(d, Ordering::Relaxed);
    if only_if_grown && d <= prev {
        return;
    }
    let top = EL1_TOP.load(Ordering::Relaxed);
    let (lo, _) = el1_bounds();
    crate::kprintln!(
        "[u91] stk el1 {} run={} depth={} B (room {} B) hz={}",
        ev,
        EL1_RUNS.load(Ordering::Relaxed),
        d,
        top - lo,
        hz()
    );
}

/// After `desktop::run()` returned.
pub fn el1_returned() {
    el1_report("returned", false);
    EL1_TOP.store(0, Ordering::Release);
}

/// The kernel's "[desk] desktop up" (EL1 or EL0 desktop with DESKTOP_ACTIVE).
pub fn desk_up() {
    if crate::launch::shell_child_slot().is_none() {
        up("el1-desktop");
        el1_report("up", false);
    }
}

// ── stack, EL0 ───────────────────────────────────────────────────────────────
static C_LO: AtomicU64 = AtomicU64::new(0); // kernel alias of the stack bottom; 0 = none
static C_TOP: AtomicU64 = AtomicU64::new(0);
static C_MAX: AtomicU64 = AtomicU64::new(0);
static C_SLOT: AtomicUsize = AtomicUsize::new(usize::MAX);
static C_INIT: AtomicBool = AtomicBool::new(false);

/// The launcher, right before the drop to EL0 (B): paint the child's crt0 stack.
pub fn el0_launch(slot: usize, basepage_va: u64) {
    T_B.store(cntvct(), Ordering::Relaxed);
    let k = |va: u64| tos_api::bdos::process::kalias(va);
    let bp = k(basepage_va) as *const u32;
    let (bbase, blen) = unsafe { (bp.add(6).read_volatile() as u64, bp.add(7).read_volatile() as u64) };
    let end = bbase + blen;
    let lo = basepage_va + ((end - basepage_va + 15) & !15);
    let top = lo + MINKEEP;
    let mut p = k(lo);
    while p < k(top) {
        unsafe { (p as *mut u64).write_volatile(MARKER) };
        p += 8;
    }
    C_LO.store(k(lo), Ordering::Relaxed);
    C_TOP.store(k(top), Ordering::Relaxed);
    C_MAX.store(0, Ordering::Relaxed);
    C_SLOT.store(slot, Ordering::Relaxed);
    C_INIT.store(false, Ordering::Relaxed);
    crate::kprintln!(
        "[u91] stk el0 painted slot={} B=0x{:x} _end=0x{:x} stack=[0x{:x},0x{:x})",
        slot,
        basepage_va,
        end,
        lo,
        top
    );
}

fn el0_report(ev: &str, only_if_grown: bool) {
    let lo = C_LO.load(Ordering::Relaxed);
    if lo == 0 {
        return;
    }
    let top = C_TOP.load(Ordering::Relaxed);
    let mut p = lo;
    while p < top && unsafe { (p as *const u64).read_volatile() } == MARKER {
        p += 8;
    }
    let d = top - p;
    let prev = C_MAX.fetch_max(d, Ordering::Relaxed);
    if only_if_grown && d <= prev {
        return;
    }
    crate::kprintln!(
        "[u91] stk el0 {} slot={} depth={} B (room {} B) hz={}",
        ev,
        C_SLOT.load(Ordering::Relaxed),
        d,
        top - lo,
        hz()
    );
}

/// Every GEM trap (top of observe_desktop_gem_call): the shell child's first evnt_multi after its
/// appl_init is its "up" (U); reads control[0] like that function.
pub fn gem_call(args: &tos_api::trap::TrapArgs) {
    use tos_api::bdos::process::{current_abi, Abi};
    let Some(child) = crate::launch::shell_child_slot() else { return };
    if !args.space.is_user()
        || tos_api::bdos::process::sched::running_slot() != child
        || args.regs[0] != 200
        || args.regs[1] == 0
    {
        return;
    }
    let control = match current_abi() {
        Abi::Lp64 => args.space.get_u64(args.regs[1]),
        Abi::Ilp32 => args.space.get_u32(args.regs[1]).map(u64::from),
    };
    let Ok(control) = control else { return };
    if control == 0 {
        return;
    }
    let Ok(op) = args.space.get_u16(control) else { return };
    match op {
        10 => C_INIT.store(true, Ordering::Relaxed),
        25 if C_INIT.swap(false, Ordering::Relaxed) => {
            up("el0-child");
            el0_report("up", false);
        }
        _ => {}
    }
}

/// The shell child terminated (X).
pub fn el0_exit() {
    T_X.store(cntvct(), Ordering::Relaxed);
    el0_report("exit", false);
    C_LO.store(0, Ordering::Relaxed);
}

static LAST: AtomicU32 = AtomicU32::new(0);

/// pump_devices: once a second, report a grown depth.
pub fn tick() {
    let now = hz();
    if now.wrapping_sub(LAST.load(Ordering::Relaxed)) < 200 {
        return;
    }
    LAST.store(now, Ordering::Relaxed);
    el1_report("tick", true);
    el0_report("tick", true);
}
