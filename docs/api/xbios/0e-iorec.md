# `Iorec` — XBIOS 0x0E

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/xbios.c:336` (`iorec`).

## NewTOS status: **decision deferred**

Returns `EINVFN` today (the XBIOS dispatcher's catch-all — defined behaviour, not
a stub). `Iorec` returns a pointer to the Atari `IOREC` ring-buffer descriptor
(IKBD/MIDI/RS-232); NewTOS has a native console-RX ring + `ikbd`. Whether to
expose an `IOREC`-shaped view of that ring is a `docs/COMPATIBILITY.md`
port-vs-reimplement call, **deferred until a program actually needs it**.

## EmuTOS source

```c
static LONG iorec(WORD devno)
{
    switch (devno) {
    case 0: return (LONG) rs232iorecptr;
    case 1: return (LONG) &ikbdiorec;
    case 2: return (LONG) &midiiorec;
    default: return -1;
    }
}
```

`IOREC` is the input ringbuffer descriptor used by the IKBD, MIDI,
and RS-232 drivers. Layout: `ibuf*` ptr / `ibufsize` / `ibufhd` /
`ibuftl` / `ibuflow` / `ibufhi`. See `docs/api/structures/iorec.md`.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x0E` |
| WORD args | 1 |
| Stack frame | `+2: devno (W)`, `+0: 0x000E` |
| Vector | `xbios_vecs[14]` |

## Inputs

| Pos | Name    | Type | Range / meaning             |
|-----|---------|------|-----------------------------|
| 0   | `devno` | WORD | 0=RS-232, 1=IKBD, 2=MIDI    |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (`IOREC *`) or `-1` |

Pointer to the device's IOREC structure — programs read/write the
ring buffer head/tail, watermark thresholds, and handshake flags
through this pointer.

## Errors

`-1` if `devno` is outside 0..2.

## Edge cases

### 1. RS-232 returns a pointer-pointer

Note `rs232iorecptr` (pointer indirection) for device 0, while
IKBD and MIDI return the address of a static struct directly.
Reason: RS-232 supports `Bconmap` device remapping (Falcon's
multiple serial ports), so the active record can change at run
time. NewTOS without Bconmap can use a fixed pointer.

### 2. Devices 3..7 not supported

Atari Falcon TOS extends `Iorec` to devices 3..7 (modem 1, modem
2, serial 1, serial 2). EmuTOS clamps to 2 and returns `-1` for
higher numbers — Compendium describes the Falcon extension that
EmuTOS does not implement. Cross-reference, not a discrepancy.

### 3. Direct buffer manipulation

Programs commonly read `ioreq->ibuftl` to detect "is data
available" without going through `Bconin`. Writing to head/tail
risks racing with the ISR — Atari convention is to disable the
ACIA interrupt around updates.

### 4. Default ringbuffer sizes

IKBD: 128 bytes. MIDI: 128 bytes. RS-232: 256 bytes (varies by TOS
version). EmuTOS uses the same sizes; programs that re-allocate
to a larger buffer write a new `ibuf` pointer + `ibufsize` and
update head/tail to 0.

## Dependencies

- `rs232iorecptr` (`bios/serport.c`).
- `ikbdiorec`, `midiiorec` (`bios/ikbd.c`, `bios/midi.c`).
- `IOREC` struct (`docs/api/structures/iorec.md`).

Used by: terminal programs, custom keyboard handlers, MIDI
applications.

## Cross-references

- **Compendium:** "Iorec" — Falcon devnos 3..7 documented.
- **Profibuch:** "Iorec (0x0E)" — agrees with EmuTOS scope.
- `docs/api/structures/iorec.md`

## Configuration

- `CONF_WITH_IKBD_ACIA`, `CONF_WITH_MIDI_ACIA`,
  `CONF_WITH_SERIAL_*`. NewTOS profile: synthesised IORECs, no
  real ACIAs.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS exposes synthetic IOREC structs whose ringbuffers
are filled by the USB-HID translator (IKBD record) and the UART
driver (RS-232 record). MIDI record present but always empty
(no underlying device).
