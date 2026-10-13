//! Board-selected console alias and shared RX ringbuffer.
//!
//! `CONSOLE` resolves to the PL011 driver instance for the active board so
//! callers can `use kernel::console::CONSOLE;` without per-board `cfg`s.
//!
//! The RX ringbuffer is the **IKBD queue** — EmuTOS' `ikbdiorec`
//! (`bios/ikbd.c:157-173`), the queue `bconstat2`/`bconin2` read. It holds
//! key **words** (`scancode << 8 | ascii`) and is consumed by the TOS-API
//! host (`host_env.rs`). Who may fill it is a per-board decision that
//! mirrors EmuTOS' `CONF_SERIAL_CONSOLE` switch — see [`serial_rx_byte`]:
//! on real Pi-4 only the IKBD decoder ([`push_rx`], fed from the USB-HID
//! keyboard); on qemu-virt additionally the PL011 RX IRQ handler
//! ([`pl011_rx_drain`], serial console — a plain ASCII byte with scancode
//! 0). PHASE-02 D5 keeps the buffer small (128 entries); a typing burst
//! that overflows simply drops keys — the GEMDOS console functions are
//! stateless w.r.t. lost input.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::hal::uart::Uart;

#[cfg(feature = "board-rpi4")]
use crate::drivers::bcm2711::pl011::CONSOLE as UART0;

#[cfg(feature = "board-qemu-virt")]
use crate::drivers::qemu_virt::pl011::CONSOLE as UART0;

// ---------------------------------------------------------------------------
// Serial output switch (SER01).
// ---------------------------------------------------------------------------

/// Whether UART0 transmits by default, before the boot arguments are read.
///
/// `board-rpi4`: **off**. Every kernel message and every byte a program
/// writes to the console used to go out synchronously; with the TX FIFO full
/// `write_byte` busy-waits (~87 us per byte at 115200 baud) and the whole
/// system stands still — for a cable that is normally not attached. The user
/// switches it on without a rebuild by putting `serial=on` into `cmdline.txt`
/// (user decision 2026-10-02, `TASK/SER01.md`); see [`configure_serial_tx`].
#[cfg(feature = "board-rpi4")]
const SERIAL_TX_DEFAULT: bool = false;

/// `board-qemu-virt`: **on**, unconditionally. The board exists only as a
/// QEMU machine, its serial log is the measurement channel of every
/// qemu-virt run, and there is no real hardware whose speed the UART could
/// cost; a switch would only add a way to lose the log.
#[cfg(feature = "board-qemu-virt")]
const SERIAL_TX_DEFAULT: bool = true;

static SERIAL_TX: AtomicBool = AtomicBool::new(SERIAL_TX_DEFAULT);

/// `true` while console bytes are actually sent to UART0.
#[inline]
pub fn serial_tx_enabled() -> bool {
    SERIAL_TX.load(Ordering::Relaxed)
}

/// Switch UART0 transmission on for good — the panic handler and the
/// fatal-exception path call this before they print, so their report always
/// leaves the machine whatever the boot arguments said. They never return,
/// so "temporarily" and "for good" are the same thing here.
pub fn force_serial_tx_on() {
    SERIAL_TX.store(true, Ordering::Relaxed);
}

/// Decide the serial switch from the boot arguments. `board-rpi4`: asks the
/// firmware for its command line (the text of `cmdline.txt`, or QEMU's
/// `-append`) through the mailbox tag "Get command line"
/// (`drivers::bcm2711::mailbox::with_command_line`) and switches TX on if it
/// contains the whole token `serial=on` ([`crate::bootargs::SERIAL_ON`]).
/// Without that token — or without any answer — the board default (off)
/// stays.
///
/// Must run right after `CONSOLE.init()` and before the first `kprint!`, so
/// nothing is sent before the decision. `entry_el` is the exception level
/// the boot stub was entered at (`1`..`3`): on this board the stub no longer
/// writes its pre-MMU heartbeat `RB<EL>` straight to the data register
/// (that byte would bypass the switch), so it is replayed here, through the
/// console, when output is on. Returns `true` if output was switched on.
#[cfg(feature = "board-rpi4")]
pub fn configure_serial_tx(entry_el: u64) -> bool {
    let on = crate::drivers::bcm2711::mailbox::with_command_line(|args| {
        crate::bootargs::has_token(args, crate::bootargs::SERIAL_ON)
    })
    .unwrap_or(false);
    if on {
        SERIAL_TX.store(true, Ordering::Relaxed);
        let el = b'0' + (entry_el & 3) as u8;
        for b in [b'R', b'B', el, b'\r', b'\n'] {
            CONSOLE.write_byte(b);
        }
    }
    on
}

/// The kernel console: UART0 of the active board behind the serial output
/// switch. Every transmit path goes through here — `kprint!`/`kprintln!`
/// (`hal::uart::UartWriter`), program console output (`host_env`), the panic
/// handler and the self-tests — so one check covers them all. Receive,
/// interrupt and init calls pass straight through: the switch is TX-only.
pub struct SerialConsole;

/// Board-selected console instance.
pub static CONSOLE: SerialConsole = SerialConsole;

impl Uart for SerialConsole {
    fn init(&self) {
        UART0.init();
    }

    #[inline]
    fn write_byte(&self, byte: u8) {
        if serial_tx_enabled() {
            UART0.write_byte(byte);
        }
    }

    fn read_byte(&self) -> Option<u8> {
        UART0.read_byte()
    }

    fn enable_rx_interrupt(&self) {
        UART0.enable_rx_interrupt();
    }

    fn clear_rx_interrupt(&self) {
        UART0.clear_rx_interrupt();
    }
}

// ---------------------------------------------------------------------------
// RX ringbuffer (single producer = IRQ handler, single consumer = host).
// ---------------------------------------------------------------------------

const RX_CAPACITY: usize = 128;

/// SPSC byte ringbuffer. Producer (IRQ handler) advances `head`; consumer
/// (Host calls) advances `tail`. Indices wrap modulo `usize`; the
/// difference `head - tail` (wrapping) gives the live byte count.
struct RxRing {
    buf: [core::cell::UnsafeCell<u16>; RX_CAPACITY],
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: SPSC discipline + atomic indices make `RxRing` safe to share
// between the IRQ handler and threaded code at EL1.
unsafe impl Sync for RxRing {}

impl RxRing {
    const fn new() -> Self {
        Self {
            // Inline const block avoids the
            // `clippy::declare_interior_mutable_const` lint that fires
            // on a named `const Z: UnsafeCell<u8>`. The two forms are
            // semantically identical — each array slot is its own
            // freshly-evaluated cell.
            buf: [const { core::cell::UnsafeCell::new(0u16) }; RX_CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&self, word: u16) {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= RX_CAPACITY {
            return; // overflow: drop key; no flow control in Phase 02.
        }
        let idx = head % RX_CAPACITY;
        // SAFETY: SPSC — only the IRQ handler writes; consumer reads
        // strictly behind `tail`, so this slot is uniquely owned here.
        unsafe { *self.buf[idx].get() = word };
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }

    fn pop(&self) -> Option<u16> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let idx = tail % RX_CAPACITY;
        // SAFETY: SPSC — slot < head, producer has finished writing.
        let w = unsafe { *self.buf[idx].get() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(w)
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    /// Non-destructive read of the oldest queued word (at `tail`) without
    /// advancing `tail`. Consumer-side only — the producer owns `head` and
    /// never touches `tail` — so it is race-free against the IRQ producer:
    /// if `head != tail` the slot at `tail` was fully written (the producer
    /// stores the slot before advancing `head` with `Release`, and we load
    /// `head` with `Acquire`).
    fn peek(&self) -> Option<u16> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let idx = tail % RX_CAPACITY;
        // SAFETY: SPSC — slot < head, producer has finished writing it; we
        // only read the slot and never advance `tail`, so no consumer state
        // changes and the producer can keep filling ahead of `head`.
        let w = unsafe { *self.buf[idx].get() };
        Some(w)
    }
}

static RX_RING: RxRing = RxRing::new();

/// Drain the PL011 RX FIFO and acknowledge the RX-side interrupt sources.
/// Called from the GIC IRQ handler when the PL011 INTID is signalled.
/// Where the received bytes go is the board's console profile —
/// [`serial_rx_byte`].
pub fn pl011_rx_drain() {
    while let Some(b) = CONSOLE.read_byte() {
        serial_rx_byte(b);
    }
    CONSOLE.clear_rx_interrupt();
}

/// Route one byte received on the serial line — the board's EmuTOS console
/// profile decides where it goes.
///
/// qemu-virt has no other console input device, so it is the
/// `CONF_SERIAL_CONSOLE = 1` profile: *"use exclusively the serial port
/// input for console input"* (`references/emutos/include/config.h:1422-1429`).
/// There EmuTOS' RS-232 receive interrupt calls `push_ascii_ikbdiorec`, i.e.
/// the byte enters the **IKBD** queue (`bios/serport.c:352-357`), which is
/// exactly what happens here: it becomes a key word with scancode 0.
#[cfg(feature = "board-qemu-virt")]
fn serial_rx_byte(b: u8) {
    #[cfg(feature = "initinfo-diag")]
    crate::initinfo::diag::note_push(crate::initinfo::diag::PUSH_SRC_SERIAL, b as u16);
    RX_RING.push(b as u16);
}

/// Real Pi-4 has its own screen and its own keyboard (USB-HID), so it is
/// EmuTOS' default `CONF_SERIAL_CONSOLE = 0` profile
/// (`references/emutos/include/config.h:1427-1429`) — the one every machine
/// with a console of its own is built with. There the RS-232 receiver pushes
/// into `iorec1`, the **RS-232 device** (BIOS handle 1,
/// `bios/serport.c:331-343` `push_serial_iorec`), and *never* into the IKBD
/// queue; `push_ascii_ikbdiorec` is not even compiled (`bios/ikbd.c:175,231`).
/// NewTOS' device profile declares handle 1 (AUX) a port with no cable —
/// status 0, input 0 (`tos_api::bios::chardev::AuxNoCable`) — so the byte has
/// no destination and is dropped here.
///
/// This is not cosmetic. UART0 is the debug console; on this board CON is the
/// USB-HID keyboard alone (STATUS 2026-07-03 K1, `c6e4bfb`), and that commit
/// left the IRQ path feeding CON only because the PL011 RX interrupt was
/// believed to be undeliverable here (`docs/MEMORY.md` QUIRK-002). The
/// 2026-09-04 boot-screen measurement disproves that: with no serial adapter
/// attached, GPIO 15 (RXD0) is an input with **no pull**
/// (`bcm2711::pl011::configure_uart0_pins`) and clocked in one spurious
/// `0x00` character — the same effect `docs/hardware/ikbd-uart.md:104` records
/// for a floating RXD2 — which arrived in this queue as a key word `0x0000`
/// before the boot screen was even drawn, and was eaten by the first
/// `Bconstat(2)`/`Bconin(2)` of the boot.
#[cfg(not(feature = "board-qemu-virt"))]
fn serial_rx_byte(_b: u8) {
    #[cfg(feature = "initinfo-diag")]
    crate::initinfo::diag::note_serial_drop(_b);
}

/// Push a key **word** into the RX ringbuffer from the IKBD decoder:
/// `scancode << 8 | ascii` (ascii 0 for keys with no character, e.g. the
/// arrow / function keys). ASCII readers (`rx_pop`) take the low byte;
/// the AES event manager reads the whole word via [`rx_pop_key`].
pub fn push_rx(word: u16) {
    #[cfg(feature = "initinfo-diag")]
    crate::initinfo::diag::note_push(crate::initinfo::diag::PUSH_SRC_IKBD, word);
    RX_RING.push(word);
}

/// Pop one **ASCII** byte from the RX ringbuffer for the console
/// byte-stream readers (`Cconrs` line input, `Fread` from CON, VDI
/// `v_locator`) — scancode-only keys (arrows etc., ascii 0) are skipped,
/// exactly as when they were never queued. The scancode-returning char
/// reads (`Bconin(2)`/`Cconin`/`Cnecin`/`Crawcin`) instead pop the whole
/// key word via [`rx_pop_key`] to preserve the high-word scancode.
pub fn rx_pop() -> Option<u8> {
    loop {
        let ascii = (RX_RING.pop()? & 0xff) as u8;
        if ascii != 0 {
            return Some(ascii);
        }
    }
}

/// Pop one whole key word (`scancode << 8 | ascii`) for the AES event
/// manager, so it can deliver scancodes (arrow / function keys) to apps.
pub fn rx_pop_key() -> Option<u16> {
    RX_RING.pop()
}

/// Non-destructively peek the oldest queued key word (`scancode << 8 |
/// ascii`) — the front element a subsequent [`rx_pop_key`] would return —
/// without consuming it. Backs the GEMDOS output-side break check
/// (`Cconout`/`Cconws` conbrk): it inspects the pending key for Ctrl-C
/// without disturbing normal type-ahead. Consumer-side, race-free against
/// the IRQ producer (see [`RxRing::peek`]).
pub fn rx_peek_key() -> Option<u16> {
    RX_RING.peek()
}

/// `true` if the RX ringbuffer holds at least one byte.
pub fn rx_has_input() -> bool {
    !RX_RING.is_empty()
}

/// The RX ringbuffer's raw producer/consumer indices — `(head, tail)`.
///
/// Diagnostic accessor for the `initinfo-diag` build: the boot-screen wait
/// loop's "a key is waiting" test is `head != tail`, so the two raw indices
/// say both *whether* something was queued and *how much* ever arrived.
#[cfg(feature = "initinfo-diag")]
pub fn rx_indices() -> (usize, usize) {
    (
        RX_RING.head.load(Ordering::Acquire),
        RX_RING.tail.load(Ordering::Acquire),
    )
}
