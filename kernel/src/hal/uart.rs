//! UART HAL trait. PHASE-01 §D3.
//!
//! Minimal byte-oriented serial output for boot diagnostics. Read support,
//! flow control, and interrupts are deliberately out of scope for this
//! phase (added in later phases when needed).

/// A byte-oriented UART used as the kernel console during boot.
pub trait Uart: Sync {
    /// Initialize the UART for 115200 baud, 8N1, FIFO enabled. Must be
    /// called exactly once before any `write_byte` / `write_str` call.
    ///
    /// Implementations also configure board-level pin muxing where
    /// required (RPi4 GPFSEL, pull-up/pull-down).
    fn init(&self);

    /// Write a single byte, blocking until the TX FIFO has room.
    fn write_byte(&self, byte: u8);

    /// Write each byte of `s` in order. Default implementation is a simple
    /// loop; drivers may override for FIFO-batched writes later.
    fn write_str(&self, s: &str) {
        for b in s.bytes() {
            self.write_byte(b);
        }
    }

    /// Try to read one byte from the RX FIFO without blocking.
    /// Returns `None` if the FIFO is empty. Default: never any input
    /// (drivers without RX support keep the default).
    fn read_byte(&self) -> Option<u8> {
        None
    }

    /// Enable the RX path and the RX / receive-timeout interrupt sources
    /// in the UART's mask register. Idempotent. Default: no-op.
    fn enable_rx_interrupt(&self) {}

    /// Acknowledge / clear the RX-side interrupt sources at the UART
    /// level. Called by the IRQ handler after draining the FIFO.
    /// Default: no-op.
    fn clear_rx_interrupt(&self) {}
}

/// Implements `core::fmt::Write` over a `Uart` reference, so callers can use
/// `write!` / `writeln!` macros without per-driver boilerplate.
pub struct UartWriter<'a, U: Uart + ?Sized>(pub &'a U);

impl<U: Uart + ?Sized> core::fmt::Write for UartWriter<'_, U> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // The one choke point every `kprint!`/`kprintln!` passes through, and
        // therefore where the `usb-diag` build tees the boot log into RAM so it
        // can be written to `C:\USBDIAG.LOG` — a real Pi 4 has no serial line
        // attached. Compiled out entirely without the feature.
        #[cfg(feature = "usb-diag")]
        crate::bootlog::capture(s);
        self.0.write_str(s);
        Ok(())
    }
}
