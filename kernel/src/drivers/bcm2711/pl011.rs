//! PL011 UART0 driver for Raspberry Pi 4 (BCM2711).
//!
//! References:
//!   - BCM2711 ARM Peripherals datasheet, §11 "PL011 UART"
//!   - BCM2711 ARM Peripherals datasheet, §5 "General Purpose I/O (GPIO)"
//!   - Andre Richter, `rust-raspberrypi-OS-tutorials` ch. 06 (PL011 init)
//!
//! Pin assignment (Raspberry Pi 4 Model B 40-pin header):
//!   - GPIO 14 (TXD0)  → header pin 8
//!   - GPIO 15 (RXD0)  → header pin 10
//!
//! Both must be set to ALT0 in `GPFSEL1`.
//!
//! Baud rate: 115200 8N1, assuming UART reference clock = 48 MHz, which is
//! the firmware default when `init_uart_clock=48000000` is configured (the
//! current RPi firmware default). Divisors:
//!   IBRD = trunc(48_000_000 / (16 * 115_200))            = 26
//!   FBRD = round((48_000_000 / (16 * 115_200) - 26) * 64) = 3

use crate::hal::uart::Uart;

// ---------------------------------------------------------------------------
// MMIO base addresses (low-peripheral mode)
// ---------------------------------------------------------------------------

/// GPIO peripheral base (high-half MMIO VA). Datasheet §5.2.
const GPIO_BASE: usize = crate::drivers::mmio_va(0xFE20_0000);
/// PL011 UART0 base (high-half MMIO VA). Datasheet §11.5.
const UART0_BASE: usize = crate::drivers::mmio_va(0xFE20_1000);

/// `GPFSEL1` — function select for GPIO 10..19. Datasheet §5.3.
const GPFSEL1: usize = GPIO_BASE + 0x04;
/// `GPIO_PUP_PDN_CNTRL_REG0` — pull-up/down control for GPIO 0..15.
/// Datasheet §5.3 (BCM2711-specific, replaces BCM2837's `GPPUD` sequence).
const GPIO_PUP_PDN_CNTRL_REG0: usize = GPIO_BASE + 0xE4;

// ---------------------------------------------------------------------------
// PL011 register offsets (datasheet §11.5)
// ---------------------------------------------------------------------------

const UART_DR:    usize = 0x00; // Data register
const UART_FR:    usize = 0x18; // Flag register
const UART_IBRD:  usize = 0x24; // Integer baud rate divisor
const UART_FBRD:  usize = 0x28; // Fractional baud rate divisor
const UART_LCRH:  usize = 0x2C; // Line control
const UART_CR:    usize = 0x30; // Control
const UART_IFLS:  usize = 0x34; // Interrupt FIFO level select
const UART_IMSC:  usize = 0x38; // Interrupt mask set/clear
const UART_ICR:   usize = 0x44; // Interrupt clear

// FR bits
const FR_TXFF: u32 = 1 << 5;    // TX FIFO full
const FR_RXFE: u32 = 1 << 4;    // RX FIFO empty

// LCRH bits
const LCRH_FEN:    u32 = 1 << 4;          // FIFO enable
const LCRH_WLEN_8: u32 = 0b11 << 5;       // 8-bit word length

// CR bits
const CR_UARTEN: u32 = 1 << 0;  // UART enable
const CR_TXE:    u32 = 1 << 8;  // TX enable
const CR_RXE:    u32 = 1 << 9;  // RX enable

// IMSC / ICR bit positions for RX-side sources.
const INT_RX: u32 = 1 << 4;     // RXIM / RXIC
const INT_RT: u32 = 1 << 6;     // RTIM / RTIC (receive timeout)

/// PL011 driver. Holds the MMIO base; safe to share via `&` because all
/// register accesses use volatile pointer ops and PL011 itself tolerates
/// concurrent TX/RX (we only use TX in this phase).
pub struct Pl011 {
    base: usize,
}

impl Pl011 {
    /// Construct a driver bound to the given MMIO base.
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    #[inline(always)]
    fn reg(&self, off: usize) -> *mut u32 {
        (self.base + off) as *mut u32
    }
}

/// Configure GPIO 14 / 15 as ALT0 (UART0 TX/RX) and disable their
/// pull-up / pull-down resistors.
///
/// # Safety
/// Touches MMIO; must run with the MMU off or the GPIO region mapped
/// device-nGnRnE. Phase 01 runs MMU-off so this is fine.
unsafe fn configure_uart0_pins() {
    // SAFETY: GPIO MMIO is valid on the RPi4 platform when the firmware has
    // brought up peripherals (always true at kernel entry).
    unsafe {
        // GPFSEL1: clear and set the 3-bit function-select fields for
        // GPIO 14 (bits [14:12]) and GPIO 15 (bits [17:15]). ALT0 = 0b100.
        let gpfsel1 = GPFSEL1 as *mut u32;
        let mut v = gpfsel1.read_volatile();
        v &= !((0b111 << 12) | (0b111 << 15));
        v |=  (0b100 << 12) | (0b100 << 15);
        gpfsel1.write_volatile(v);

        // GPIO_PUP_PDN_CNTRL_REG0: 2 bits per pin.
        //   GPIO 14 → bits [29:28], GPIO 15 → bits [31:30].
        //   0b00 = none, 0b01 = pull-up, 0b10 = pull-down.
        let pud = GPIO_PUP_PDN_CNTRL_REG0 as *mut u32;
        let mut p = pud.read_volatile();
        p &= !((0b11u32 << 28) | (0b11u32 << 30));
        pud.write_volatile(p);
    }
}

impl Uart for Pl011 {
    fn init(&self) {
        // SAFETY: PL011 / GPIO MMIO is valid; sequence follows datasheet.
        unsafe {
            configure_uart0_pins();

            // Disable UART before changing baud rate / line settings.
            self.reg(UART_CR).write_volatile(0);

            // Clear all pending interrupts.
            self.reg(UART_ICR).write_volatile(0x7FF);

            // Baud divisors for 115200 @ 48 MHz (see module docs).
            self.reg(UART_IBRD).write_volatile(26);
            self.reg(UART_FBRD).write_volatile(3);

            // 8N1, FIFO enabled.
            self.reg(UART_LCRH).write_volatile(LCRH_FEN | LCRH_WLEN_8);

            // Enable UART, TX, RX.
            self.reg(UART_CR).write_volatile(CR_UARTEN | CR_TXE | CR_RXE);
        }
    }

    fn write_byte(&self, byte: u8) {
        // SAFETY: same as `init` — well-defined PL011 register access.
        unsafe {
            while self.reg(UART_FR).read_volatile() & FR_TXFF != 0 {
                core::hint::spin_loop();
            }
            self.reg(UART_DR).write_volatile(byte as u32);
        }
    }

    fn read_byte(&self) -> Option<u8> {
        // SAFETY: see `init`.
        unsafe {
            if self.reg(UART_FR).read_volatile() & FR_RXFE != 0 {
                None
            } else {
                Some(self.reg(UART_DR).read_volatile() as u8)
            }
        }
    }

    fn enable_rx_interrupt(&self) {
        // SAFETY: see `init`.
        unsafe {
            // 1/8 RX-FIFO trigger level (= 4 bytes on a 32-byte FIFO).
            self.reg(UART_IFLS).write_volatile(0b000 << 3);
            // Unmask RX + receive-timeout.
            let prev = self.reg(UART_IMSC).read_volatile();
            self.reg(UART_IMSC).write_volatile(prev | INT_RX | INT_RT);
        }
    }

    fn clear_rx_interrupt(&self) {
        // SAFETY: see `init`.
        unsafe {
            self.reg(UART_ICR).write_volatile(INT_RX | INT_RT);
        }
    }
}

/// Global console instance for this board.
pub static CONSOLE: Pl011 = Pl011::new(UART0_BASE);
