//! Kernel-side implementation of [`tos_api::Host`].
//!
//! The host wires TOS-API console operations to the board's PL011
//! console driver. Phase 02 only needs output; the read paths are
//! stubbed out (returning `None` / `false`) until the IRQ-driven
//! ringbuffer in PHASE-02 D5 lands.

use tos_api::Host;

use crate::console::CONSOLE;
use crate::hal::uart::Uart;

/// Singleton host installed into `tos-api` during boot.
pub struct KernelHost;

impl Host for KernelHost {
    fn console_write_byte(&self, byte: u8) {
        CONSOLE.write_byte(byte);
    }

    fn console_write_bytes(&self, s: &[u8]) {
        for &b in s {
            CONSOLE.write_byte(b);
        }
    }

    fn console_try_read(&self) -> Option<u8> {
        crate::console::rx_pop()
    }

    fn console_has_input(&self) -> bool {
        crate::console::rx_has_input()
    }

    fn console_read_byte_blocking(&self) -> u8 {
        // This runs inside the synchronous SVC handler, where the CPU
        // took the exception with IRQs masked — so the PL011 RX IRQ
        // that fills the ringbuffer can never fire and a plain spin
        // would deadlock. Save DAIF, unmask IRQ (`daifclr #2` clears
        // the `I` bit), `wfi` until a byte lands in the ringbuffer,
        // then restore the original mask.
        let daif: u64;
        // SAFETY: reading DAIF and clearing the IRQ mask are permitted
        // at EL1; we restore DAIF before returning.
        unsafe {
            core::arch::asm!(
                "mrs {0}, daif",
                out(reg) daif,
                options(nomem, nostack, preserves_flags),
            );
            core::arch::asm!(
                "msr daifclr, #2",
                options(nomem, nostack, preserves_flags),
            );
        }

        let byte = loop {
            if let Some(b) = crate::console::rx_pop() {
                break b;
            }
            // SAFETY: wait for the next interrupt (the RX IRQ wakes us).
            unsafe {
                core::arch::asm!("wfi", options(nomem, nostack, preserves_flags));
            }
        };

        // SAFETY: restore the caller's original DAIF mask state.
        unsafe {
            core::arch::asm!(
                "msr daif, {0}",
                in(reg) daif,
                options(nomem, nostack, preserves_flags),
            );
        }
        byte
    }

    fn monotonic_seconds(&self) -> u64 {
        // ARM generic timer: physical count / frequency (Hz) = seconds
        // since the counter started (boot). Both are EL0-readable
        // system registers (ARM ARM D17.6 "Generic Timer").
        let cnt: u64;
        let frq: u64;
        // SAFETY: read-only system-register reads, always permitted.
        unsafe {
            core::arch::asm!(
                "mrs {cnt}, cntpct_el0",
                "mrs {frq}, cntfrq_el0",
                cnt = out(reg) cnt,
                frq = out(reg) frq,
                options(nomem, nostack, preserves_flags),
            );
        }
        cnt.checked_div(frq).unwrap_or(0)
    }

    fn raster(&self) -> Option<&dyn tos_api::vdi::backend::RasterBackend> {
        // Hand the VDI the software framebuffer backend, but only once a
        // framebuffer is actually up (geometry published).
        if crate::vdi_backend::available() {
            Some(&crate::vdi_backend::VDI_SCREEN)
        } else {
            None
        }
    }
}

/// Static reference handed to `tos_api::host::install_host`.
static KERNEL_HOST: KernelHost = KernelHost;
static KERNEL_HOST_REF: &'static dyn Host = &KERNEL_HOST;

/// Install the kernel host into `tos-api`. Called once from
/// `kernel_main`.
pub fn install() {
    tos_api::host::install_host(&KERNEL_HOST_REF);
}
