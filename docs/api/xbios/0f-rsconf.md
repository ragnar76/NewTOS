# `Rsconf` — XBIOS 0x0F

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/xbios.c:376` (`rsconf`).

## EmuTOS source

```c
static ULONG rsconf(WORD baud, WORD ctrl, WORD ucr, WORD rsr,
                    WORD tsr, WORD scr)
{
    return (*rsconfptr)(baud, ctrl, ucr, rsr, tsr, scr);
}
```

`rsconfptr` is a function pointer set by `bios/serport.c` at boot
to the active serial port's configuration routine (`rsconf1` for
the MFP USART, or one of the Falcon SCC variants).

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x0F` |
| WORD args | 6 (WORD × 6) |
| Stack frame | `+12: scr (W)`, `+10: tsr (W)`, `+8: rsr (W)`, `+6: ucr (W)`, `+4: ctrl (W)`, `+2: baud (W)`, `+0: 0x000F` |
| Vector | `xbios_vecs[15]` |

## Inputs

| Pos | Name   | Type | Meaning                                                    |
|-----|--------|------|------------------------------------------------------------|
| 0   | `baud` | WORD | baud-rate index (0..15), `-1` = no change                   |
| 1   | `ctrl` | WORD | flow-control mode, `-1` = no change                         |
| 2   | `ucr`  | WORD | MFP 68901 USART control register (parity / stop / bits)     |
| 3   | `rsr`  | WORD | MFP 68901 receiver status register                         |
| 4   | `tsr`  | WORD | MFP 68901 transmitter status register                      |
| 5   | `scr`  | WORD | MFP 68901 sync character register                          |

Each `-1` argument leaves the corresponding hardware register
untouched. `baud == -1` means "query current".

### `baud` index (Atari MFP convention)

| Index | Baud  | Index | Baud   |
|------:|------:|------:|-------:|
| 0     | 19200 | 8     | 600    |
| 1     | 9600  | 9     | 300    |
| 2     | 4800  | 10    | 200    |
| 3     | 3600  | 11    | 150    |
| 4     | 2400  | 12    | 134    |
| 5     | 2000  | 13    | 110    |
| 6     | 1800  | 14    | 75     |
| 7     | 1200  | 15    | 50     |

### `ctrl` flow-control modes

| Value | Meaning            |
|------:|--------------------|
| 0     | No flow control     |
| 1     | XON/XOFF           |
| 2     | RTS/CTS            |
| 3     | XON/XOFF + RTS/CTS  |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | ULONG |

The previous `(ucr << 24) | (rsr << 16) | (tsr << 8) | scr` packed
register state — exact format depends on the active serial driver.
Backward compatibility: programs read the lower 16 bits as
"previous baud rate" when only `baud` was the argument of
interest.

## Errors

None — invalid arguments are silently passed to hardware. An
`-1`-only call (read-current) cannot fail.

## Edge cases

### 1. MFP-register arguments are MFP-specific

`ucr`/`rsr`/`tsr`/`scr` map directly onto the 68901 MFP USART
registers. NewTOS without MFP hardware must define an equivalent
mapping for the AArch64 PL011 UART; the obvious approach is to
ignore these on the input side and synthesise plausible values on
output.

### 2. Indirection via `rsconfptr`

`rsconfptr` is updated by `Bconmap` to switch among Falcon's
multiple serial ports. NewTOS without Bconmap maps the single
PL011 UART to device 0 and ignores `Bconmap` calls.

### 3. UART vs Atari quirks

The Atari MFP flow-control implementation has known issues with
high baud rates (loses bytes above 9600 without RTS/CTS).
NewTOS PL011 + DMA does not have this limitation; faster baud
rates are fine even without flow control.

### 4. Console redirect implication

Some programs `Rsconf(N, ...)` and then `Bconout(1, ...)` to send
data over the modem. The console-device routing happens in
`bconmap` — `Rsconf` itself only configures.

## Dependencies

- `rsconfptr` (`bios/serport.c`).
- MFP / SCC hardware.

Used by: terminal programs, modem dialers, MIDI-over-serial
adaptors.

## Cross-references

- **Compendium:** "Rsconf" — agrees on baud table & flow modes.
- **Profibuch:** "Rsconf (0x0F)" — agrees, lists MFP register
  layouts.
- `docs/api/xbios/2c-bconmap.md` (pending).

## Configuration

- `CONF_WITH_MFP`, `CONF_WITH_SCC`. NewTOS profile: neither;
  PL011 UART substitute.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS implements `rsconf` against PL011 UART. Baud
table maps to PL011 IBRD/FBRD divisor registers. `ctrl` flow-
control bits map to UART CR.RTSEn/CTSEn. MFP register arguments
ignored on input; output packs synthesised values for ABI
compatibility.
