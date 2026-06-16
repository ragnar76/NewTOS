# `Cconos` — GEMDOS 0x10

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xconostat()`.

## EmuTOS source

- **Primary:** `bdos/console.c:164` — `xconostat()`
- **Helpers:** `Bcostat` (BIOS 0x08)
- **Globals read:** `run->p_uft[1]`

## Source extraction

```c
long xconostat(void)
{
    return Bcostat(HXFORM(run->p_uft[1]));
}
```

Pure pass-through: report whether the output device referenced by
stdout's standard handle is ready to accept a byte.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0010` |
| WORD args | 0 |

## Inputs

None.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |

| Hardware state           | Value |
|--------------------------|-------|
| Output device ready      | `-1L` |
| Output device not ready  | `0L`  |

## Errors

None.

## Edge cases

- **Standard-handle redirection** (`p_uft[1] > 0`): dispatcher returns
  `-1L` unconditionally (file is always writable).

## Dependencies

- `Bcostat(h)`, `run->p_uft[1]`

Used by: programs throttling output to slow devices.

## Cross-references

- **Compendium:** "Cconos" — agrees.
- **Profibuch:** "Cconos (16)" — agrees.

## Configuration / version

None.

## NewTOS implementation notes

(Empty during Phase 00.)
