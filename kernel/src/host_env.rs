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
}

/// Static reference handed to `tos_api::host::install_host`.
static KERNEL_HOST: KernelHost = KernelHost;
static KERNEL_HOST_REF: &'static dyn Host = &KERNEL_HOST;

/// Install the kernel host into `tos-api`. Called once from
/// `kernel_main`.
pub fn install() {
    tos_api::host::install_host(&KERNEL_HOST_REF);
}
