# Track-B reference: second PL011 UART for the Atari IKBD keyboard

> Status: **values to be cross-checked against the BCM2711 datasheet
> before connecting hardware.** They follow the established BCM2711 UART
> pattern already verified for UART0 in `kernel/src/drivers/bcm2711/pl011.rs`,
> but were not re-read from the datasheet in the environment that wrote
> this driver — confirm the UART2 base and the GPIO pin/ALT choice for
> your board + wiring first.

## Purpose

A native Atari ST keyboard (HD6301 IKBD) streams a serial protocol at
**7812.5 baud, 8N1** (key make/break scancodes + mouse/joystick/clock
packets). Wiring it to a spare Pi UART gives authentic Atari scancodes
with no USB stack. The protocol decoder is `kernel/src/ikbd.rs`; this
file documents the UART that feeds it.

## Baud rate

PL011 baud = `UARTCLK / (16 × divisor)`. With the firmware default
`init_uart_clock=48000000` (already set in the SD card's `config.txt`):

```
48_000_000 / (16 × 7812.5) = 384.0   → IBRD = 384, FBRD = 0  (exact)
```

No fractional part — 7812.5 baud is hit exactly.

## Controller

The BCM2711 has several PL011 UARTs (datasheet §11.5). UART0 (the
console) is at `0xFE20_1000`; the additional PL011s follow at +0x200
spacing:

| UART | Base (low-peripheral mode) |
|------|----------------------------|
| UART0 | `0xFE20_1000` (console)   |
| UART2 | `0xFE20_1400`             |
| UART3 | `0xFE20_1600`             |
| UART4 | `0xFE20_1800`             |
| UART5 | `0xFE20_1A00`             |

This driver uses **UART2 @ `0xFE20_1400`**. The register layout is
identical to UART0 (see `pl011.rs`).

## GPIO routing

UART2 TXD/RXD map to **GPIO 0 / GPIO 1, ALT4** (datasheet §5.3 GPIO
alternate-function table). On the 40-pin header:

- GPIO 0 (TXD2) → header pin 27
- GPIO 1 (RXD2) → header pin 28

For **keyboard input only** you need just RXD2 (GPIO 1, pin 28): the
keyboard's TX line goes through a **5 V → 3.3 V divider** (1 kΩ series +
2 kΩ to GND) into pin 28. Power the keyboard from the header's +5 V
(pin 2/4) and GND. The Pi→keyboard direction (TXD2) is only needed later
for mouse-mode commands and would need a proper level shifter.

The 3-bit GPFSEL function value for ALT4 is `0b011`. GPIO 0/1 live in
`GPFSEL0` (`GPIO_BASE + 0x00`): GPIO 0 = bits [2:0], GPIO 1 = bits [5:3].

## Wakeup

The driver currently **polls** UART2 (no GIC IRQ wired for it). At
7812.5 baud that is cheap; an IRQ-driven path can replace it once the
hardware is connected and the UART2 GIC SPI is confirmed.
