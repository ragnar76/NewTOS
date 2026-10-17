//! BCM2711 power manager (PM) watchdog — the full-chip restart (TASK/RST01).
//!
//! Reference: `docs/hardware/pm-watchdog-reset.md`.
//!
//! **Source level, stated plainly:** the BCM2711 ARM Peripherals datasheet
//! (`references/datasheets/RP-008248-DS-1-bcm2711-peripherals.pdf`) does
//! **not** document the power manager or its watchdog (searched in full,
//! 2026-10-17). Every address and value in this file comes from a
//! **tertiary** source — Circle (`references/circle`, `6177984e`):
//! `include/circle/bcm2835.h:261-279` (block base, register offsets,
//! password) and `lib/sysinit.cpp:231-241` (`reboot()`, the write sequence) —
//! cross-checked against QEMU's model of the same block
//! (`hw/misc/bcm2835_powermgt.c`, online master, same offsets and password).
//! It is not a datasheet fact; the user accepted that residual risk for the
//! cold restart (RST01 decision 1, 2026-10-17).

/// Power-manager block, low-peripheral mode: `ARM_IO_BASE (0xFE00_0000) +
/// 0x10_0000` for `RASPPI <= 4` (Circle `bcm2835.h:29-30,261-265`).
pub const BCM2711_PM_BASE: usize = 0xFE10_0000;

/// Reset control register, `PM_BASE + 0x1C` (Circle `bcm2835.h:267`,
/// `ARM_PM_RSTC`; QEMU `R_RSTC 0x1c`).
pub const BCM2711_PM_RSTC: usize = BCM2711_PM_BASE + 0x1C;

/// Watchdog countdown register, `PM_BASE + 0x24` (Circle `bcm2835.h:269`,
/// `ARM_PM_WDOG`; QEMU `R_WDOG 0x24`).
pub const BCM2711_PM_WDOG: usize = BCM2711_PM_BASE + 0x24;

/// Write password, required in bits 31:24 of every PM register write; a
/// write without it is ignored (Circle `bcm2835.h:274`, `ARM_PM_PASSWD
/// (0x5A << 24)`; QEMU `PASSWORD 0x5a000000`, mask `0xff000000`).
pub const BCM2711_PM_PASSWD: u32 = 0x5A << 24;

/// `RSTC` "full reset" configuration: arm the watchdog to reset the chip when
/// the `WDOG` count expires (Circle `sysinit.cpp:236`,
/// `PM_RSTC_WRCFG_FULL_RESET 0x20`; QEMU acts on `(rstc & 0x30) == 0x20`).
pub const BCM2711_PM_RSTC_WRCFG_FULL_RESET: u32 = 0x20;

/// The watchdog count Circle's `reboot()` loads — "set some timeout", the
/// shortest non-zero value (`sysinit.cpp:233`). A count of 0 would not arm
/// the reset at all (QEMU: `WDOG` 0 cancels the timer).
pub const BCM2711_PM_WDOG_REBOOT_COUNT: u32 = 1;

/// Restart the whole SoC through the PM watchdog — Circle `reboot()`
/// (`lib/sysinit.cpp:231-241`): load `WDOG` with a short count, then select
/// the full-reset configuration in `RSTC`; the chip resets when the count
/// runs out, and the firmware boots `kernel8.img` again. Never returns.
///
/// Callers clean the data cache first (EmuTOS `cold_reset`,
/// `bios/panicasm.S:133`); this function only touches the two registers.
pub fn reboot() -> ! {
    let wdog = crate::drivers::mmio_va(BCM2711_PM_WDOG) as *mut u32;
    let rstc = crate::drivers::mmio_va(BCM2711_PM_RSTC) as *mut u32;
    // SAFETY: both registers lie in the low-peripheral window
    // `[0xFE00_0000, 0xFF00_0000)`, which the kernel maps Device-nGnRnE in
    // the high half (`bios::machine::layout` rpi4 `MMIO`); 32-bit aligned
    // MMIO stores. Circle brackets the access with `PeripheralEntry()` — a
    // barrier — hence the `dsb sy` before the first store.
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(wdog, BCM2711_PM_PASSWD | BCM2711_PM_WDOG_REBOOT_COUNT);
        core::ptr::write_volatile(rstc, BCM2711_PM_PASSWD | BCM2711_PM_RSTC_WRCFG_FULL_RESET);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    // "wait for reset" (Circle `sysinit.cpp:239`).
    crate::arch::aarch64::halt()
}
