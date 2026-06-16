# `Ikbdws` — XBIOS 0x19

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/ikbd.c:948` (`ikbdws`).

## EmuTOS source

```c
void ikbdws(WORD cnt, const UBYTE *ptr)
{
    do { ikbd_writeb(*ptr++); } while (cnt--);
}
```

`ikbd_writeb` polls the IKBD ACIA's transmit-ready flag, then
writes one byte to `ikbd_acia.data`. On Amiga / Coldfire / ColdFire
FlexCAN the transport differs but the polling loop is identical.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x19` |
| WORD args | 4 (WORD + LONG) |
| Stack frame | `+4: ptr (L)`, `+2: cnt (W)`, `+0: 0x0019` |
| Vector | `xbios_vecs[25]` |

## Inputs

| Pos | Name  | Type            | Range / meaning                              |
|-----|-------|-----------------|----------------------------------------------|
| 0   | `cnt` | WORD            | byte count **less one** (treated unsigned)    |
| 1   | `ptr` | const UBYTE *   | command bytes (IKBD command-set encoding)     |

## Outputs

`void`. D0 == 0 from dispatcher default.

## Errors

None — `ikbd_writeb` blocks until success.

## Edge cases

### 1. cnt is "count minus one" (same as Midiws)

`Ikbdws(0, ptr)` sends one byte. `Ikbdws(-1, ptr)` sends 65 536
bytes. Quirk inherited from Atari TOS.

### 2. Tiny-stack accommodation

The EmuTOS source comment notes that "New Beat's 4kB Falcon demo
'Blue' calls Ikbdws() with sp == 0x22". The implementation is
hand-tuned to use minimal stack. NewTOS must preserve this
constraint when re-implementing — keep the stack frame ≤ 16 bytes.

### 3. IKBD command set

The bytes form Atari IKBD commands: `0x07` set mouse-button mode,
`0x14` set joystick mode, `0x1B` set clock, etc. Sending a clock-
set command (`0x1B yyyymmddhhmmss`) and then immediately reading
via `Bconin(4)` is a documented idiom for RTC programming —
preserved in EmuTOS's `isetdt`.

### 4. Polling bcostat4

Each byte waits for `bcostat4` (BIOS Bcostat for device 4 = IKBD).
A wedged IKBD blocks indefinitely. NewTOS implementation must
include a watchdog timeout returning silently after, e.g., 100 ms.

### 5. Race with IKBD interrupt

The IKBD ACIA RX interrupt continues firing during `ikbdws` — a
mouse move while sending a command leaves a mouse packet in the
IORC. Programs receiving an IKBD reply (e.g. clock read) must
tolerate inter-leaved mouse / joystick packets.

## Dependencies

- `ikbd_writeb` ⇒ IKBD ACIA hardware.
- `bcostat4` (BIOS Bcostat device 4).

Used by: clock setters (`isetdt`), custom mouse-mode setup,
joystick / paddle mode switches.

## Cross-references

- **Compendium:** "Ikbdws" — agrees on cnt-1 quirk.
- **Profibuch:** "Ikbdws (0x19)" — agrees.
- `docs/api/xbios/0c-midiws.md` — same byte-stream pattern.

## Configuration

- `CONF_WITH_IKBD_ACIA` for ST-family hardware. NewTOS profile:
  no IKBD chip.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS translates IKBD command bytes into NewTOS-
internal calls (clock-set → RTC driver, mouse-mode → USB-HID
config). Unrecognised commands are silently dropped; this matches
the Atari behaviour for unsupported IKBD commands.
