//! Board-selected console alias and shared RX ringbuffer.
//!
//! `CONSOLE` resolves to the PL011 driver instance for the active board so
//! callers can `use kernel::console::CONSOLE;` without per-board `cfg`s.
//!
//! The RX ringbuffer is filled from the PL011 IRQ handler
//! ([`pl011_rx_drain`]) and consumed by the TOS-API host
//! (`host_env.rs`). PHASE-02 D5 keeps the buffer small (128 bytes); a
//! typing burst that overflows simply drops bytes — the GEMDOS
//! console functions are stateless w.r.t. lost input.

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::hal::uart::Uart;

#[cfg(feature = "board-rpi4")]
pub use crate::drivers::bcm2711::pl011::CONSOLE;

#[cfg(feature = "board-qemu-virt")]
pub use crate::drivers::qemu_virt::pl011::CONSOLE;

// ---------------------------------------------------------------------------
// RX ringbuffer (single producer = IRQ handler, single consumer = host).
// ---------------------------------------------------------------------------

const RX_CAPACITY: usize = 128;

/// SPSC byte ringbuffer. Producer (IRQ handler) advances `head`; consumer
/// (Host calls) advances `tail`. Indices wrap modulo `usize`; the
/// difference `head - tail` (wrapping) gives the live byte count.
struct RxRing {
    buf: [core::cell::UnsafeCell<u8>; RX_CAPACITY],
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
            buf: [const { core::cell::UnsafeCell::new(0u8) }; RX_CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&self, byte: u8) {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= RX_CAPACITY {
            return; // overflow: drop byte; no flow control in Phase 02.
        }
        let idx = head % RX_CAPACITY;
        // SAFETY: SPSC — only the IRQ handler writes; consumer reads
        // strictly behind `tail`, so this slot is uniquely owned here.
        unsafe { *self.buf[idx].get() = byte };
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }

    fn pop(&self) -> Option<u8> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let idx = tail % RX_CAPACITY;
        // SAFETY: SPSC — slot < head, producer has finished writing.
        let b = unsafe { *self.buf[idx].get() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(b)
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }
}

static RX_RING: RxRing = RxRing::new();

/// Drain the PL011 RX FIFO into the ringbuffer and acknowledge the
/// RX-side interrupt sources. Called from the GIC IRQ handler when the
/// PL011 INTID is signalled.
pub fn pl011_rx_drain() {
    while let Some(b) = CONSOLE.read_byte() {
        RX_RING.push(b);
    }
    CONSOLE.clear_rx_interrupt();
}

/// Push a byte into the RX ringbuffer from a non-UART input source (the
/// IKBD keyboard decoder). The byte is then read by `Cnecin`/`Cconin`
/// exactly like a UART byte.
pub fn push_rx(byte: u8) {
    RX_RING.push(byte);
}

/// Pop one byte from the RX ringbuffer, if any.
pub fn rx_pop() -> Option<u8> {
    RX_RING.pop()
}

/// `true` if the RX ringbuffer holds at least one byte.
pub fn rx_has_input() -> bool {
    !RX_RING.is_empty()
}
