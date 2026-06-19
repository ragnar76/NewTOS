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
        crate::fbcon::write_byte(byte);
    }

    fn console_write_bytes(&self, s: &[u8]) {
        for &b in s {
            CONSOLE.write_byte(b);
            crate::fbcon::write_byte(b);
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
            // Drain a native Atari IKBD keyboard (second UART, no IRQ
            // wired) into the same ring. board-rpi4 only.
            #[cfg(feature = "board-rpi4")]
            crate::drivers::bcm2711::ikbd_uart::poll();

            // Also poll the UART0 console RX FIFO directly. The PL011 RX
            // IRQ path is verified only in QEMU; on real GIC-400 silicon
            // it is not yet confirmed to be delivered, which would leave
            // the cable console with no input. Polling here mirrors the
            // IKBD poll above and feeds the same ring.
            #[cfg(feature = "board-rpi4")]
            crate::console::pl011_rx_poll();

            if let Some(b) = crate::console::rx_pop() {
                break b;
            }
            // On board-rpi4 the IKBD UART is polled, so spin lightly
            // rather than sleeping (only the UART0 RX IRQ would wake a
            // `wfi`, never the polled keyboard). On other boards `wfi`
            // sleeps until the RX IRQ.
            #[cfg(feature = "board-rpi4")]
            core::hint::spin_loop();
            #[cfg(not(feature = "board-rpi4"))]
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

    fn frame_clock(&self) -> u64 {
        // CNTPCT * 60 / CNTFRQ = the current ~60 Hz frame number.
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
        cnt.checked_mul(60).unwrap_or(cnt).checked_div(frq).unwrap_or(0)
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

    fn screen(&self) -> Option<tos_api::host::ScreenInfo> {
        if !crate::vdi_backend::available() {
            return None;
        }
        let (width, height) = crate::vdi_backend::dims();
        Some(tos_api::host::ScreenInfo {
            base: crate::vdi_backend::base_addr(),
            width,
            height,
            bpp: 32,
        })
    }

    fn keyboard_shift_state(&self) -> u8 {
        crate::ikbd::shift_state()
    }

    #[cfg(feature = "board-rpi4")]
    fn block(&self) -> Option<&dyn tos_api::bios::block::BlockDev> {
        use crate::hal::block::BlockDevice;
        if crate::drivers::bcm2711::emmc::SDCARD.block_count() > 0 {
            Some(&KERNEL_BLOCK)
        } else {
            None
        }
    }
}

/// Adapter exposing the EMMC2 SD card as a TOS-API [`BlockDev`].
#[cfg(feature = "board-rpi4")]
struct KernelBlock;

#[cfg(feature = "board-rpi4")]
impl tos_api::bios::block::BlockDev for KernelBlock {
    fn block_count(&self) -> u64 {
        use crate::hal::block::BlockDevice;
        crate::drivers::bcm2711::emmc::SDCARD.block_count()
    }
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> bool {
        use crate::hal::block::BlockDevice;
        crate::drivers::bcm2711::emmc::SDCARD.read_blocks(lba, buf).is_ok()
    }
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> bool {
        use crate::hal::block::BlockDevice;
        crate::drivers::bcm2711::emmc::SDCARD.write_blocks(lba, buf).is_ok()
    }
}

#[cfg(feature = "board-rpi4")]
static KERNEL_BLOCK: KernelBlock = KernelBlock;

/// Static reference handed to `tos_api::host::install_host`.
static KERNEL_HOST: KernelHost = KernelHost;
static KERNEL_HOST_REF: &'static dyn Host = &KERNEL_HOST;

/// Install the kernel host into `tos-api`. Called once from
/// `kernel_main`.
pub fn install() {
    tos_api::host::install_host(&KERNEL_HOST_REF);
}
