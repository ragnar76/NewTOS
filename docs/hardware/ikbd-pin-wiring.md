# IKBD keyboard/mouse — Raspberry Pi 4 pin wiring

> Which Pi-4 40-pin-header pins to use to connect a native Atari IKBD
> keyboard (+ its mouse) to NewTOS over UART2, plus the serial-console
> pins for diagnostics. Companion: [`ikbd-uart.md`](ikbd-uart.md) (driver
> details) and [`desktop-hw-test-plan.md`](desktop-hw-test-plan.md) (tests).

## Verification status (read first)

| Fact | Source | Trust |
|---|---|---|
| Physical header pin ↔ GPIO number | Raspberry Pi 40-pin standard | ✅ reliable, universal |
| GPIO0 → **TXD2** / GPIO1 → **RXD2** on **ALT4** | BCM2711 ARM Peripherals datasheet (`references/datasheets/RP-008248-DS-1-bcm2711-peripherals.pdf`), **Table 94, §5.3 "Alternative Function Assignments" (p.78)**, reconfirmed **Table 171 (p.146)** | ✅ datasheet-verified 2026-06-25 (matches `ikbd_uart.rs`) |
| UART2 register base `0x7e20_1400` (= `0xFE20_1400` low-peripheral) | same datasheet, p.146 | ✅ datasheet-verified |

The GPIO0/1 → UART2/ALT4 routing the driver programs is now confirmed
against the datasheet (Table 94 column header: `ALT0 ALT1 ALT2 ALT3 ALT4
ALT5`; GPIO0's ALT4 = `TXD2`, GPIO1's ALT4 = `RXD2`), so it is no longer a
suspect for "no input". The physical pin numbers below are the Raspberry Pi
40-pin standard.

## ⚠️ Voltage — do not skip

The IKBD signals at **5 V**; Pi GPIO is **3.3 V tolerant only**. The
keyboard's TX must reach the Pi through a divider; never put 5 V directly
on a GPIO.

```
Keyboard TX (5 V logic) ──[ 1 kΩ ]──┬── Pi GPIO1 / RXD2  (header pin 28)
                                     │
                                   [ 2 kΩ ]
                                     │
                                    GND  (e.g. header pin 25)

  node voltage = 5 V × 2k / (1k + 2k) = 3.33 V   ✓ safe for the Pi
```

For **keyboard + mouse you only need the Keyboard→Pi (RX) direction**, so
the divider above is enough. The Pi→Keyboard (TX) direction is only needed
to send IKBD commands (e.g. mouse-mode / Y-direction) and would need a
proper 3.3 V→5 V level shifter — not required to start.

## Pins to use (Pi 4 / 40-pin header)

| Purpose | Header pin | GPIO / function | Wire to |
|---|---|---|---|
| **Keyboard data → Pi** (RXD2) | **pin 28** | GPIO1, ALT4 = RXD2 | keyboard **TX**, via the 1k/2k divider |
| Keyboard power +5 V | **pin 2** (or 4) | 5 V | keyboard **+5 V** |
| Ground (common) | **pin 25** (or 6/9/…) | GND | keyboard **GND** + divider GND |
| Pi → keyboard (TXD2, optional) | pin 27 | GPIO0, ALT4 = TXD2 | keyboard **RX**, via a level shifter (only for mouse-mode cmds) |
| Serial console TX (diagnostics) | pin 8 | GPIO14, TXD0 | USB-serial **RX** |
| Serial console RX (diagnostics) | pin 10 | GPIO15, RXD0 | USB-serial **TX** |
| Serial console GND | pin 6 | GND | USB-serial **GND** |

### Header map (relevant pins highlighted)

```
            3V3  ( 1) ( 2)  5V    ← +5V (keyboard power)
          GPIO2  ( 3) ( 4)  5V
          GPIO3  ( 5) ( 6)  GND   ← console GND
          GPIO4  ( 7) ( 8)  GPIO14 TXD0   ← console TX
            GND  ( 9) (10)  GPIO15 RXD0   ← console RX
         GPIO17  (11) (12)  GPIO18
         GPIO27  (13) (14)  GND
         GPIO22  (15) (16)  GPIO23
            3V3  (17) (18)  GPIO24
         GPIO10  (19) (20)  GND
          GPIO9  (21) (22)  GPIO25
         GPIO11  (23) (24)  GPIO8
            GND  (25) (26)  GPIO7        ← (25) keyboard/divider GND
   TXD2  GPIO0   (27) (28)  GPIO1  RXD2  ← (27) Pi→kbd opt, (28) kbd→Pi
          GPIO5  (29) (30)  GND
          GPIO6  (31) (32)  GPIO12
         GPIO13  (33) (34)  GND
         GPIO19  (35) (36)  GPIO16
         GPIO26  (37) (38)  GPIO20
            GND  (39) (40)  GPIO21
```

## Firmware config (`config.txt` on the SD boot partition)

UART2's PL011 clock must be the 48 MHz the driver's baud divisor assumes
(`IBRD = 384` for 7812.5 baud). Ensure the boot `config.txt` has:

```
init_uart_clock=48000000
```

The kernel programs the GPIO0/1 ALT4 mux itself, so no `dtoverlay` is
strictly required; if the firmware parks the pins, add `dtoverlay=uart2`.

## Quick connection summary

1. Keyboard **GND** → Pi **pin 25** (or any GND).
2. Keyboard **+5 V** → Pi **pin 2**.
3. Keyboard **TX** → `[1 kΩ]` → Pi **pin 28** (RXD2); from that same node `[2 kΩ]` → GND.
4. (Optional) Pi **pin 27** (TXD2) → level shifter → keyboard **RX**.
5. USB-serial adapter to **pins 8/10/6** at 115200 8N1 to watch `[desk] …`.

Then run the [test plan](desktop-hw-test-plan.md) starting at test 0b
(move the mouse → cursor should move).
