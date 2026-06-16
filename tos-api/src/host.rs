//! Host abstraction.
//!
//! TOS-API code does not call kernel internals directly. Instead the
//! kernel installs a `&'static dyn Host` once at boot via
//! [`install_host`]; every TOS function then routes through this
//! interface. This keeps `tos-api` independent of the concrete UART /
//! GIC / framebuffer drivers and makes the surface unit-testable.

use core::sync::atomic::{AtomicPtr, Ordering};

/// Kernel-supplied host environment.
///
/// All methods may be called from inside an exception handler with
/// interrupts disabled — implementations must therefore be lock-free
/// or use IRQ-safe locks. Phase 02 only requires console I/O.
pub trait Host: Sync {
    /// Write a single byte to the console output device.
    fn console_write_byte(&self, byte: u8);

    /// Write each byte of `s` to the console output device. Default
    /// implementation loops over [`Host::console_write_byte`].
    fn console_write_bytes(&self, s: &[u8]) {
        for &b in s {
            self.console_write_byte(b);
        }
    }

    /// Non-blocking read from the console input device. Returns
    /// `Some(byte)` if a character is available, `None` otherwise.
    fn console_try_read(&self) -> Option<u8> {
        None
    }

    /// Returns `true` if the console has at least one byte ready.
    fn console_has_input(&self) -> bool {
        false
    }

    /// **Blocking** read of one byte from the console input device —
    /// waits until input is available. Used by the blocking GEMDOS
    /// input functions (`Cconin`, `Cnecin`, `Crawcin`, `Cconrs`).
    ///
    /// The default busy-polls [`Host::console_try_read`]; the kernel
    /// host overrides it to enable IRQs and `wfi` while it waits (the
    /// PL011 RX interrupt fills the ringbuffer, which is otherwise
    /// masked inside the synchronous SVC handler).
    fn console_read_byte_blocking(&self) -> u8 {
        loop {
            if let Some(b) = self.console_try_read() {
                return b;
            }
            core::hint::spin_loop();
        }
    }
}

// `&'static dyn Host` is `Sync + 'static`; we store it as a raw
// pointer because there is no `AtomicRef`. Access is read-mostly:
// installed exactly once at boot, never replaced.
static HOST: AtomicPtr<&'static dyn Host> = AtomicPtr::new(core::ptr::null_mut());

/// Install the kernel-supplied host. Must be called exactly once
/// before any trap is taken.
///
/// # Panics
///
/// Panics in debug builds if called more than once.
pub fn install_host(host: &'static &'static dyn Host) {
    let ptr = host as *const _ as *mut _;
    let prev = HOST.swap(ptr, Ordering::Release);
    debug_assert!(prev.is_null(), "tos_api::host already installed");
}

/// Access the installed host. Returns `None` before [`install_host`]
/// has run; the dispatcher treats that as a fatal misconfiguration.
#[inline]
pub fn host() -> Option<&'static dyn Host> {
    let ptr = HOST.load(Ordering::Acquire);
    if ptr.is_null() {
        None
    } else {
        // SAFETY: `install_host` stored a valid `&'static &'static dyn Host`.
        Some(unsafe { *ptr })
    }
}
