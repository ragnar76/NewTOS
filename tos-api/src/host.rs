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

    /// Whole seconds elapsed since boot, from a monotonic hardware
    /// counter. The GEMDOS time functions add this to a settable base
    /// epoch to produce the wall clock. The kernel host reads the ARM
    /// generic timer (`CNTPCT_EL0 / CNTFRQ_EL0`); the default `0` makes
    /// the clock stand still (acceptable fallback for a hostless test).
    fn monotonic_seconds(&self) -> u64 {
        0
    }

    /// The VDI rasterizer backend (the screen device), or `None` if the
    /// board has no framebuffer. This is the swappable hardware boundary
    /// for the VDI: the bring-up host returns a software rasterizer over
    /// the framebuffer; a later accelerated (NEON / DMA / GPU) backend
    /// drops in here without the VDI layer changing. Default `None`
    /// keeps hostless / display-less configurations building.
    fn raster(&self) -> Option<&dyn crate::vdi::backend::RasterBackend> {
        None
    }

    /// The block device backing BIOS `Rwabs` (the SD card), or `None` if
    /// the board has no storage. Same swappable-boundary pattern as
    /// [`Host::raster`].
    fn block(&self) -> Option<&dyn crate::bios::block::BlockDev> {
        None
    }

    /// The current hardware framebuffer (base address + geometry), or
    /// `None` if no display is up. Backs the XBIOS screen group
    /// (`Physbase` / `Logbase` / `Getrez`). Same swappable boundary as
    /// [`Host::raster`].
    fn screen(&self) -> Option<ScreenInfo> {
        None
    }
}

/// The hardware framebuffer as the XBIOS screen calls see it.
#[derive(Clone, Copy)]
pub struct ScreenInfo {
    /// CPU-visible base address of the framebuffer (the drawing target).
    pub base: u64,
    /// Visible width in pixels.
    pub width: u16,
    /// Visible height in pixels.
    pub height: u16,
    /// Bits per pixel (32 for our XRGB8888 framebuffer).
    pub bpp: u16,
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
