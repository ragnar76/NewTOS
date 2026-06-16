# `Cauxos` — GEMDOS 0x13

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xauxostat()`.

## EmuTOS source

- **Primary:** `bdos/console.c:188` — `xauxostat()`
- **Helpers:** `Bcostat` (BIOS 0x08)
- **Globals read:** `run->p_uft[2]`

## Source extraction

```c
long xauxostat(void)
{
    return Bcostat(HXFORM(run->p_uft[2]));
}
```

Pure pass-through.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0013` |
| WORD args | 0 |

## Inputs

None.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Value | `-1L` if AUX output device ready, else `0L`. |

## Errors

None.

## Edge cases

- **Standard-handle redirection** (`p_uft[2] > 0`): dispatcher returns
  `-1L`.

## Dependencies

- `Bcostat(h)`, `run->p_uft[2]`

Used by: programs throttling RS-232 output.

## Cross-references

- **Compendium:** "Cauxos" — agrees.
- **Profibuch:** "Cauxos (19)" — agrees.

## Configuration / version

- AUX BIOS device: `CONF_WITH_MFP`, `CONF_WITH_SCC`.

## NewTOS implementation notes

(Empty during Phase 00.)
