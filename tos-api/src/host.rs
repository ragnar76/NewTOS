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

    /// Move the **logical** screen — where drawing goes, EmuTOS `v_bas_ad`
    /// (`bios/screen.c:1106-1107`) — to the screen page whose CPU-visible
    /// base is `base`. Returns `false` when `base` is not the base of a page
    /// of this display's screen memory, in which case nothing moved.
    ///
    /// The default `false` is the honest answer of a host with no display:
    /// there is no screen to move. It is *not* a "silently accepted" no-op —
    /// the caller learns the move did not happen and
    /// [`Host::screen`]`().base` keeps reporting where drawing really goes.
    fn set_screen_logical(&self, _base: u64) -> bool {
        false
    }

    /// Move the **physical** screen — what the display engine scans out,
    /// EmuTOS `setphys()` (`bios/screen.c:1110-1111`) — to the screen page
    /// whose CPU-visible base is `base`. Returns `false` when the hardware
    /// cannot show that address; the scan-out is then left exactly where it
    /// was and [`Host::screen`]`().phys_base` keeps reporting the truth, the
    /// same way EmuTOS' `physbase()` reads the video-base register back
    /// rather than a shadow.
    fn set_screen_physical(&self, _base: u64) -> bool {
        false
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

    /// The CPU core clock the machine is running at, in **Hz**, or `None`
    /// when this board cannot be asked. Backs `Ssystem(S_NEWTOS_CPUCLOCK)`
    /// (`docs/api/gemdos/154-ssystem.md`). Same swappable hardware boundary
    /// as [`Host::raster`]: on `board-rpi4` the kernel host asks the
    /// VideoCore firmware over the mailbox property channel; a board without
    /// that channel (`board-qemu-virt`, a hostless test) keeps the default
    /// and answers honestly with `None`, which the caller turns into
    /// `ERROR`. It never invents a number — a clock frequency that was not
    /// measured is not a clock frequency.
    fn cpu_clock_hz(&self) -> Option<u32> {
        None
    }

    /// The SoC's current die temperature in **thousandths of a degree
    /// Celsius** (47.2 °C → `47200`), or `None` when this board has no
    /// temperature sensor to ask. Backs `Ssystem(S_NEWTOS_CPUTEMP)`
    /// (`docs/api/gemdos/154-ssystem.md`); same boundary and the same
    /// honest-`None` rule as [`Host::cpu_clock_hz`].
    ///
    /// Signed because a temperature is: the firmware reports the value as an
    /// unsigned word, but a board started below freezing is a physical state,
    /// not an error, and the TOS-side LONG has to be able to carry it.
    fn cpu_temperature_mdeg(&self) -> Option<i32> {
        None
    }
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
///
/// Three addresses, because a display whose screen memory holds more than one
/// screen has three (EmuTOS `bios/screen.c`):
///
/// * [`Self::mem_base`] / [`Self::mem_bytes`] — the whole allocated screen
///   memory, `mem_bytes / size_bytes` screen pages of it. This is the extent
///   a program that draws into the screen must be able to reach, and the
///   extent `Setscreen` may point the two bases into.
/// * [`Self::base`] — the **logical** screen: where drawing goes
///   (`v_bas_ad`).
/// * [`Self::phys_base`] — the **physical** screen: what the display engine
///   scans out (`physbase()`, read back from the hardware).
///
/// On a single-page display all three coincide.
#[derive(Clone, Copy)]
pub struct ScreenInfo {
    /// CPU-visible base address of the **logical** screen — the drawing
    /// target, EmuTOS `v_bas_ad`.
    pub base: u64,
    /// CPU-visible base address of the **physical** screen — the page the
    /// display engine is currently scanning out.
    pub phys_base: u64,
    /// CPU-visible base of the whole screen memory (page 0).
    pub mem_base: u64,
    /// Byte size of the whole screen memory, `size_bytes` × the number of
    /// screen pages it holds.
    pub mem_bytes: u32,
    /// Visible width in pixels.
    pub width: u16,
    /// Visible height in pixels.
    pub height: u16,
    /// Bits per pixel (32 for our XRGB8888 framebuffer).
    pub bpp: u16,
    /// Bytes per scan line (the hardware pitch). May exceed
    /// `width * bpp / 8` when the display controller pads lines.
    pub stride_bytes: u32,
    /// Byte size of **one** screen (`stride_bytes * height`) — the distance
    /// between two screen pages inside [`Self::mem_bytes`].
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
