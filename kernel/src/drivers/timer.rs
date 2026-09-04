//! ARM Generic Timer — the NewTOS 200 Hz system tick.
//!
//! This is the hardware half of the TOS system clock: it produces the
//! periodic interrupt that drives [`tos_api::bios::tick`], whose counter is
//! the `_hz_200` system variable (`$4BA`). On an Atari the source is MFP
//! timer C (`references/emutos/bios/mfp.c:193-209`, `xbtimer(2, 0x50, 192, …)`
//! = 2 457 600/64/192 = 200 Hz); on AArch64 it is the EL1 physical timer of
//! the ARM generic timer, which is the same clock NewTOS already reads for
//! `monotonic_millis` (`kernel/src/host_env.rs`).
//!
//! # Sources (Track B)
//!
//! * **Which interrupt.** The EL1 *non-secure physical* timer's output is a
//!   private per-core interrupt (PPI) with **INTID 30** on both target
//!   boards:
//!   - BCM2711 ARM Peripherals datasheet §6.3, Figure 7 "GIC IRQ routing":
//!     `Core n PNS timer IRQ → PPI ID 30` (the same figure lists HP = 26,
//!     V = 27, PS = 29 and `SPI IDs 32-47` above it, so these are absolute
//!     GIC INTIDs).
//!   - QEMU `virt` (the exact machine `xtask run` starts, QEMU 11.0.2,
//!     `-M virt,gic-version=2`): its own device tree describes
//!     `/timer { compatible = "arm,armv8-timer"; interrupts = <1 13 0x104>,
//!     <1 14 0x104>, <1 11 0x104>, <1 10 0x104>; }` — secure-physical,
//!     non-secure-physical, virtual, hypervisor — i.e. PPI 14 for the one we
//!     use, and PPI *n* is INTID 16 + *n*, giving 30. The four numbers map
//!     onto the BCM2711 figure's 29/30/27/26 exactly.
//!   - Cross-check (tertiary): `references/circle/include/circle/bcm2711int.h:26,30`
//!     — `GIC_PPI(n) (16 + (n))`, `ARM_IRQLOCAL0_CNTPNS GIC_PPI(14)`.
//! * **Register usage.** `CNTFRQ_EL0` (counter frequency), `CNTPCT_EL0`
//!   (free-running count), `CNTP_CVAL_EL0` (absolute compare value) and
//!   `CNTP_CTL_EL0` (bit 0 = enable, unmasked) — ARM ARM D17.6 "Generic
//!   Timer", the section `host_env.rs` already cites for `CNTPCT_EL0` /
//!   `CNTFRQ_EL0`. The ARM ARM itself is not in `references/`; the working
//!   pattern used here (write `CNTP_CVAL_EL0` = deadline, then
//!   `CNTP_CTL_EL0` = 1) is the one in `references/circle/lib/timer.cpp:126-131`.
//! * **EL1 access.** The boot code sets `CNTHCTL_EL2.{EL1PCTEN,EL1PCEN}`
//!   before dropping EL2 → EL1 (`arch/aarch64/boot.rs:281-284`), so both the
//!   counter and the physical *timer* registers are accessible at EL1 on the
//!   RPi4 path; QEMU `virt` hands us EL1 directly with no EL2 trap.

use core::sync::atomic::{AtomicU64, Ordering};

/// GIC INTID of the EL1 non-secure physical timer interrupt (PPI 14).
/// Identical on both boards — see the module docs for the two primary
/// sources.
pub const TIMER_INTID: u32 = 30;

/// System-tick rate: 200 Hz, the Atari timer-C rate
/// ([`tos_api::bios::tick::TICK_HZ`]).
const TICK_HZ: u64 = tos_api::bios::tick::TICK_HZ as u64;

/// `CNTP_CTL_EL0.ENABLE` (bit 0). Bit 1 (`IMASK`) stays clear so the timer
/// asserts its interrupt output.
const CNTP_CTL_ENABLE: u64 = 1;

/// Counter ticks per 200 Hz period (`CNTFRQ_EL0 / 200`), or `0` before
/// [`init`] has run / when the machine reports no counter frequency.
static PERIOD: AtomicU64 = AtomicU64::new(0);

/// Read `CNTFRQ_EL0` — the generic timer's frequency in Hz.
#[inline]
fn cntfrq() -> u64 {
    let v: u64;
    // SAFETY: read-only system-register read, permitted at EL1.
    unsafe {
        core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) v,
                         options(nomem, nostack, preserves_flags));
    }
    v
}

/// Read `CNTPCT_EL0` — the free-running physical counter.
#[inline]
fn cntpct() -> u64 {
    let v: u64;
    // SAFETY: read-only system-register read, permitted at EL1.
    unsafe {
        core::arch::asm!("mrs {0}, cntpct_el0", out(reg) v,
                         options(nomem, nostack, preserves_flags));
    }
    v
}

/// Read the physical timer's absolute compare value `CNTP_CVAL_EL0`.
#[inline]
fn cval() -> u64 {
    let v: u64;
    // SAFETY: read-only system-register read; the EL2→EL1 drop enabled EL1
    // access to the physical timer registers (`boot.rs:281-284`).
    unsafe {
        core::arch::asm!("mrs {0}, cntp_cval_el0", out(reg) v,
                         options(nomem, nostack, preserves_flags));
    }
    v
}

/// Set the physical timer's absolute compare value. The interrupt is
/// asserted while `CNTPCT_EL0 >= CNTP_CVAL_EL0`, so writing a deadline in
/// the future also de-asserts it.
#[inline]
fn set_cval(v: u64) {
    // SAFETY: writing the EL1 physical timer compare register; permitted at
    // EL1 per `CNTHCTL_EL2.EL1PCEN` (set in `boot.rs`).
    unsafe {
        core::arch::asm!("msr cntp_cval_el0, {0}", in(reg) v,
                         options(nomem, nostack, preserves_flags));
    }
}

/// Write `CNTP_CTL_EL0` and context-synchronise, so the new enable/disable
/// state is in effect before the caller continues.
#[inline]
fn set_ctl(v: u64) {
    // SAFETY: as `set_cval`; `isb` is the architectural context
    // synchronisation after a system-register write.
    unsafe {
        core::arch::asm!("msr cntp_ctl_el0, {0}", "isb", in(reg) v,
                         options(nomem, nostack, preserves_flags));
    }
}

/// Start the 200 Hz system tick: unmask the timer PPI in the GIC, arm the
/// first deadline and enable the timer.
///
/// Returns `false` — with the timer left off — when the machine reports a
/// counter frequency of 0 or one below 200 Hz, i.e. when no 5 ms period can
/// be expressed. The caller reports that; `_hz_200` then stays at 0 rather
/// than counting at a wrong rate.
///
/// Call with IRQs still masked (`DAIF.I` set); the boot sequence unmasks
/// them a few lines later.
pub fn init() -> bool {
    let frq = cntfrq();
    let period = frq / TICK_HZ;
    if period == 0 {
        return false;
    }
    PERIOD.store(period, Ordering::Relaxed);

    super::gic::board_gic().enable_ppi(TIMER_INTID);
    set_cval(cntpct().wrapping_add(period));
    set_ctl(CNTP_CTL_ENABLE);
    true
}

/// Timer interrupt handler, dispatched from [`super::gic::rust_irq_handler`]
/// on [`TIMER_INTID`]. Re-arms the timer and credits the elapsed periods to
/// the `_hz_200` counter.
///
/// The deadline is advanced from the *previous* deadline, not from "now", so
/// the tick cannot drift; and a late interrupt (IRQs are masked inside the
/// synchronous SVC handler, which can run a whole disk transfer) credits
/// every period it covered instead of losing them.
pub fn handle_irq() {
    let period = PERIOD.load(Ordering::Relaxed);
    if period == 0 {
        // Never armed by `init` — silence the (level-sensitive) source
        // instead of returning into an interrupt storm.
        set_ctl(0);
        return;
    }

    let deadline = cval();
    let now = cntpct();
    // The interrupt fires at `now >= deadline`; the excess is the delivery
    // lateness. One tick for this period, plus one for every whole period
    // that went by unserviced.
    let ticks = 1 + now.saturating_sub(deadline) / period;
    set_cval(deadline.wrapping_add(ticks.wrapping_mul(period)));

    tos_api::bios::tick::advance(ticks as u32);
}
