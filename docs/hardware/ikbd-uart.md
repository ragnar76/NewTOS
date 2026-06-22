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

## Software status (2026-06-22)

The full software path is **complete and verified in QEMU**:

- `kernel/src/ikbd.rs` decodes relative-mouse packets (`0xF8..0xFB`: low 2
  header bits = buttons, swapped to VDI convention, then signed `dx`/`dy`)
  and feeds `tos_api::vdi::feed_relative` → the VDI cursor.
- `kernel/src/drivers/bcm2711/ikbd_uart::init()` runs at boot and `poll()`
  drains UART2 RX into the decoder (called from `host_env::pump_input`).
- End-to-end proof (`desktop::preview_ikbd_click`): raw IKBD packets fed to
  `ikbd::feed` walk the cursor onto a drive icon and press the button; the
  desktop `evnt_multi` loop opens the directory window. So once real bytes
  arrive on UART2, the desktop is interactive — no further software needed
  for the basic case.

## Remaining for real hardware (open — needs the datasheet + a keyboard)

Per CLAUDE.md Rule 6 these must be confirmed against the **BCM2711 ARM
Peripherals datasheet** (not currently in `references/`) before trusting
them on hardware:

1. **UART2 base `0xFE20_1400` and GPIO 0/1 ALT4 routing** — re-read from
   the datasheet §5.3 alt-function table and §11 UART map. The values here
   follow the known BCM2711 pattern but were not read from the datasheet in
   this environment.
2. **Mouse Y direction.** The IKBD defaults to "Y = 0 at bottom"; the
   screen has Y = 0 at top, so `dy` may need inverting. EmuTOS configures
   this with IKBD command `0x0F` (Y at bottom) / `0x10` (Y at top). The
   decoder currently applies `dy` un-inverted — confirm the sign on real
   hardware and, if needed, invert (or send `0x10` over TXD2).
3. **Mouse enable.** After reset the IKBD streams relative-mouse reports by
   default, so RXD2-only wiring should work; sending "set relative mouse"
   (`0x08`) over TXD2 makes it deterministic but needs a proper 3.3 V→5 V
   level shifter on the Pi→keyboard line.
4. **RXD2 idle level.** `init()` disables the pull on GPIO 1; a floating
   RXD2 with no keyboard attached can clock in noise. Consider a pull-up
   (UART idle = mark/high) once the wiring is finalised.
