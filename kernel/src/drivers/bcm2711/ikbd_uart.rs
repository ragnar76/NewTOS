//! Second PL011 UART for a native Atari ST IKBD keyboard (7812.5 baud).
//!
//! Track-B reference: `docs/hardware/ikbd-uart.md`. The register layout
//! is the PL011 (same as `pl011.rs`); only the base, GPIO routing and
//! baud divisor differ. Received bytes are handed to the IKBD protocol
//! decoder (`crate::ikbd`).
//!
//! **The base and GPIO values below follow the established BCM2711 UART
//! pattern but should be cross-checked against the datasheet for your
//! board/wiring before connecting hardware.**

use crate::ikbd;

/// GPIO peripheral base (datasheet §5.2), as in `pl011.rs`.
const GPIO_BASE: usize = 0xFE20_0000;
/// `GPFSEL0` — function select for GPIO 0..9 (datasheet §5.3, offset 0).
const GPFSEL0: usize = GPIO_BASE;
/// Pull-up/down control for GPIO 0..15 (datasheet §5.3).
const GPIO_PUP_PDN_CNTRL_REG0: usize = GPIO_BASE + 0xE4;

/// PL011 UART2 base (datasheet §11.5; UART0 + 0x400).
const UART2_BASE: usize = 0xFE20_1400;

// PL011 register offsets (datasheet §11.5).
const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_IBRD: usize = 0x24;
const UART_FBRD: usize = 0x28;
const UART_LCRH: usize = 0x2C;
const UART_CR: usize = 0x30;
const UART_ICR: usize = 0x44;

const FR_RXFE: u32 = 1 << 4; // RX FIFO empty
const LCRH_FEN: u32 = 1 << 4;
const LCRH_WLEN_8: u32 = 0b11 << 5;
const CR_UARTEN: u32 = 1 << 0;
const CR_TXE: u32 = 1 << 8;
const CR_RXE: u32 = 1 << 9;

#[inline(always)]
fn reg(off: usize) -> *mut u32 {
    (UART2_BASE + off) as *mut u32
}

/// Configure UART2 (GPIO 0/1, ALT4) at 7812.5 baud, 8N1, RX enabled.
///
/// # Safety
/// Touches GPIO + UART2 MMIO; valid once the firmware has brought up
/// peripherals (always true at kernel entry).
pub unsafe fn init() {
    unsafe {
        // GPIO 0/1 → ALT4 (0b011): GPIO0 = GPFSEL0[2:0], GPIO1 = [5:3].
        let gpfsel0 = GPFSEL0 as *mut u32;
        let mut v = gpfsel0.read_volatile();
        v &= !(0b111 | (0b111 << 3));
        v |= 0b011 | (0b011 << 3);
        gpfsel0.write_volatile(v);

        // Disable pull-up/down on GPIO 0/1 (bits [1:0] and [3:2]).
        let pud = GPIO_PUP_PDN_CNTRL_REG0 as *mut u32;
        let mut p = pud.read_volatile();
        p &= !(0b11u32 | (0b11u32 << 2));
        pud.write_volatile(p);

        // Configure the UART.
        reg(UART_CR).write_volatile(0);
        reg(UART_ICR).write_volatile(0x7FF);
        // 7812.5 baud @ 48 MHz UARTCLK: IBRD = 384, FBRD = 0 (exact).
        reg(UART_IBRD).write_volatile(384);
        reg(UART_FBRD).write_volatile(0);
        reg(UART_LCRH).write_volatile(LCRH_FEN | LCRH_WLEN_8);
        reg(UART_CR).write_volatile(CR_UARTEN | CR_TXE | CR_RXE);
    }
}

/// Drain any pending RX bytes from UART2 into the IKBD decoder. Cheap to
/// call in the console input-wait loop.
///
/// The drain is bounded to the PL011 FIFO depth (32) so that a platform
/// which does not implement UART2 (its `FR` reads 0, leaving `RXFE`
/// permanently clear — e.g. QEMU `raspi4b`) cannot spin forever; unread
/// bytes are simply picked up on the next call.
pub fn poll() {
    // SAFETY: well-defined PL011 register reads on the configured UART.
    unsafe {
        for _ in 0..32 {
            if reg(UART_FR).read_volatile() & FR_RXFE != 0 {
                break;
            }
            let b = reg(UART_DR).read_volatile() as u8;
            ikbd::feed(b);
        }
    }
}
