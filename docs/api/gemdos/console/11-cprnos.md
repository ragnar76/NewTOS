# `Cprnos` — GEMDOS 0x11

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xprtostat()`.

## EmuTOS source

- **Primary:** `bdos/console.c:172` — `xprtostat()`
- **Helpers:** `Bcostat` (BIOS 0x08)
- **Globals read:** `run->p_uft[3]`
- **Build config:** `CONF_WITH_PRINTER_PORT` (transitive).

## Source extraction

```c
long xprtostat(void)
{
    return Bcostat(HXFORM(run->p_uft[3]));
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0011` |
| WORD args | 0 |

## Inputs

None.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Value | `-1L` if printer ready (parport BUSY low + SELECT high), else `0L`. |

## Errors

None.

## Edge cases

- **Standard-handle redirection** (`p_uft[3] > 0`): dispatcher returns
  `-1L`.
- **No printer attached / parport disabled (`CONF_WITH_PRINTER_PORT=0`)**:
  `Bcostat(0)` returns `0L` permanently.

## Dependencies

- `Bcostat(h)`, `run->p_uft[3]`

Used by: well-behaved printer-output loops.

## Cross-references

- **Compendium:** "Cprnos" — agrees.
- **Profibuch:** "Cprnos (17)" — agrees.

## Configuration / version

- `CONF_WITH_PRINTER_PORT`.

## NewTOS implementation notes

(Empty during Phase 00.)
