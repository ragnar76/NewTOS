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

    /// Non-blocking read of one whole **key word** (`scancode << 8 | ascii`)
    /// for the AES event manager, so it can deliver scancode-only keys
    /// (arrows, function keys). The default has no scancode source and
    /// widens the ASCII byte (scancode 0); the kernel host overrides this
    /// with the IKBD key ring.
    fn console_try_read_key(&self) -> Option<u16> {
        self.console_try_read().map(|b| b as u16)
    }

    /// Non-blocking, **non-destructive** peek of the next whole key word
    /// (`scancode << 8 | ascii`) at the front of the console input queue —
    /// the value the next [`Host::console_try_read_key`] would return —
    /// without consuming it. Used by the GEMDOS output-side break check
    /// (`Cconout`/`Cconws` conbrk) to spot a pending Ctrl-C without
    /// disturbing normal type-ahead. The default has no peekable source
    /// (`None`); the kernel host overrides it with the IKBD key ring.
    fn console_peek_key(&self) -> Option<u16> {
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

    /// **Blocking** read of one whole **key word** (`scancode << 8 | ascii`)
    /// from the console input device — waits until a key is available. Unlike
    /// [`Host::console_read_byte_blocking`] this does **not** skip
    /// scancode-only keys (ASCII 0, e.g. arrows / function keys): those must
    /// be delivered so `Bconin(2)`/`Cconin`/`Cnecin`/`Crawcin` can return the
    /// packed console LONG (scancode in bits 16-23, ASCII in bits 0-7 — see
    /// `docs/api/bios/02-bconin.md`).
    ///
    /// The default busy-polls [`Host::console_try_read_key`]; the kernel host
    /// overrides it to enable IRQs and spin/`wfi` while it waits (mirroring
    /// `console_read_byte_blocking`).
    fn console_read_key_blocking(&self) -> u16 {
        loop {
            if let Some(w) = self.console_try_read_key() {
                return w;
            }
            core::hint::spin_loop();
        }
    }

    /// Advance the polled input sources once (keyboard/mouse bytes from
    /// the UART(s)), with interrupts briefly enabled. The AES event
    /// manager calls this while it polls for a wait condition: an
    /// `evnt_*` runs in the synchronous SVC handler with IRQs masked, so
    /// without this the RX sources never advance and the wait would
    /// deadlock. Default: no-op (a hostless test has no hardware to pump).
    fn pump_input(&self) {}

    /// Whole seconds elapsed since boot, from a monotonic hardware
    /// counter. The GEMDOS time functions add this to a settable base
    /// epoch to produce the wall clock. The kernel host reads the ARM
    /// generic timer (`CNTPCT_EL0 / CNTFRQ_EL0`); the default `0` makes
    /// the clock stand still (acceptable fallback for a hostless test).
    fn monotonic_seconds(&self) -> u64 {
        0
    }

    /// Milliseconds elapsed since boot, from the same monotonic hardware
    /// counter as [`Host::monotonic_seconds`] but at millisecond
    /// resolution (`CNTPCT * 1000 / CNTFRQ`). Backs the AES event timer
    /// (`evnt_timer` / `evnt_multi`'s `MU_TIMER`). The default `0` makes
    /// a timed wait fire immediately (acceptable for a hostless test).
    fn monotonic_millis(&self) -> u64 {
        0
    }

    /// A monotonic **frame counter** at ~60 Hz, derived from the hardware
    /// timer (`CNTPCT * 60 / CNTFRQ`). Backs XBIOS `Vsync` (spin until it
    /// ticks) and seeds `Random`. The default `0` makes `Vsync` a no-op
    /// (acceptable for a hostless test).
    fn frame_clock(&self) -> u64 {
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

    /// The current keyboard shift-state bitmask (`MODE_*` bits: `0x01`
    /// right-shift, `0x02` left-shift, `0x04` ctrl, `0x08` alt, `0x10`
    /// caps), as BIOS `Kbshift` and VDI `vq_key_s` report it. The kernel
    /// host reads the IKBD decoder's modifier state; the default `0`
    /// (no modifiers) suits a hostless / keyboard-less configuration.
    fn keyboard_shift_state(&self) -> u8 {
        0
    }

    /// Overwrite the keyboard shift-state bitmask (same `MODE_*` bits as
    /// [`Host::keyboard_shift_state`]) — the set half of BIOS `Kbshift`
    /// (`flag >= 0`, EmuTOS `ikbd.c:150` stores `flag` into the `shifty`
    /// byte). The kernel host writes the IKBD decoder's modifier state so
    /// a subsequent [`Host::keyboard_shift_state`] reads the value back;
    /// the default is a no-op (a hostless / keyboard-less configuration
    /// has no modifier state to set). Not a stub — a `Kbshift` set with no
    /// backing keyboard simply has nothing to store, exactly as EmuTOS'
    /// `shifty` write is inert with no ACIA present.
    fn set_keyboard_shift_state(&self, _mask: u8) {}

    /// The three primary scancode→ASCII translation tables plus the AltGr
    /// override arrays of the **active** keyboard layout, in the shape XBIOS
    /// `Keytbl` publishes them (EmuTOS `struct keytbl`, `bios/ikbd.h:57-86`).
    /// `None` when the board has no keyboard translator, in which case
    /// `Keytbl` has nothing to point at. Same swappable boundary as
    /// [`Host::raster`].
    fn key_tables(&self) -> Option<KeyTables> {
        None
    }

    /// Install a caller-supplied 128-byte scancode→ASCII table as one of the
    /// three primary tables — the set half of `Keytbl` (EmuTOS
    /// `bios/ikbd.c:124-132` assigns the pointer; NewTOS's translator owns a
    /// RAM layout, so the bytes are taken over instead of the pointer). The
    /// default is a no-op: a board with no keyboard translator has no table
    /// to replace.
    fn set_key_table(&self, _which: KeyTable, _table: &[u8; 128]) {}
}

/// Bytes reserved for one AltGr override array. EmuTOS's non-`DUAL_KEYBOARD`
/// format is `(scancode, ascii)` pairs terminated by a zero byte
/// (`bios/ikbd.h:66-71`); NewTOS's translator holds up to 16 pairs per shift
/// level, so 33 bytes always suffice.
pub const ALT_TABLE_LEN: usize = 33;

/// Which of the three primary tables [`Host::set_key_table`] replaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyTable {
    /// Unshifted (`norm`).
    Norm,
    /// Shifted (`shft`).
    Shft,
    /// Caps-locked (`caps`).
    Caps,
}

/// A snapshot of the active keyboard layout in `struct keytbl` shape
/// (`bios/ikbd.h:57-86`). The three primary tables are 128-byte
/// scancode→ASCII arrays; the three AltGr arrays are zero-terminated
/// `(scancode, ascii)` pair lists padded to [`ALT_TABLE_LEN`].
#[derive(Clone, Copy)]
pub struct KeyTables {
    /// Unshifted table.
    pub norm: [u8; 128],
    /// Shifted table.
    pub shft: [u8; 128],
    /// Caps-locked table.
    pub caps: [u8; 128],
    /// AltGr overrides for the unshifted level.
    pub altnorm: [u8; ALT_TABLE_LEN],
    /// AltGr overrides for the shifted level.
    pub altshft: [u8; ALT_TABLE_LEN],
    /// AltGr overrides for the caps-locked level.
    pub altcaps: [u8; ALT_TABLE_LEN],
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
    /// Bytes per scan line (the hardware pitch). May exceed
    /// `width * bpp / 8` when the display controller pads lines.
    pub stride_bytes: u32,
    /// Byte size of the CPU-visible pixel store (`stride_bytes * height`) —
    /// the span `Physbase`/`Logbase` must make reachable for a caller that
    /// draws into the screen directly.
    pub size_bytes: u32,
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
