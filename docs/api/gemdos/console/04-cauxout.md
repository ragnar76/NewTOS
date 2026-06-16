# `Cauxout` — GEMDOS 0x04

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xauxout()`.

## EmuTOS source

- **Primary:** `bdos/console.c:329` — `xauxout()`
- **Helpers:** `Bconout` (BIOS 0x03)
- **Globals read:** `run->p_uft[2]`
- **Build config:** transitive only.

## Source extraction

```c
long xauxout(int ch)
{
    return Bconout(HXFORM(run->p_uft[2]), ch);
}
```

Pure pass-through. No tab expansion, no `glbcolumn` tracking, no
flow-control probe.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0004` |
| WORD args | 1 |
| Stack frame | `+2: ch (WORD)`, `+0: 0x0004` |

## Inputs

| Pos | Name | Type | Notes                                   |
|-----|------|------|-----------------------------------------|
| 0   | `ch` | WORD | low 8 bits used                         |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Encoding | whatever `Bconout` returns. EmuTOS's `Bconout` returns 0 on success or a small negative on driver-defined error; in practice 0. |

## Errors

None standardised. Underlying RS-232 driver may report buffer-overrun
or hardware-not-ready as a non-zero return, but typical NewTOS code
should treat any non-zero as success-or-driver-private.

## Edge cases

- **Blocking on full transmit buffer**: depends on the RS-232 driver
  back-end. EmuTOS's default driver waits for the UART transmit-empty
  flag, so `Cauxout` is effectively blocking until the byte is
  shifted out.
- **Standard-handle redirection** (`run->p_uft[2] > 0`): dispatcher
  writes the byte to a file via `xwrite()`; this function not entered.

## Dependencies

- `Bconout(h, c)`
- `run->p_uft[2]`

Used by: programs writing to RS-232 / current AUX device.

## Cross-references

- **Compendium:** "Cauxout" — agrees.
- **Profibuch:** "Cauxout (4)" — agrees.

## Configuration / version

- `CONF_WITH_MFP`, `CONF_WITH_SCC`: drive the AUX BIOS device.

## NewTOS implementation notes

(Empty during Phase 00.)
