# `Cconis` — GEMDOS 0x0B

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xconstat()`.

## EmuTOS source

- **Primary:** `bdos/console.c:155` — `xconstat()`
- **Helpers:** `constat()` (l.144), `Bconstat` (BIOS 0x01)
- **Globals read:** `run->p_uft[0]`, typeahead state

## Source extraction

```c
long xconstat(void)
{
    return constat(HXFORM(run->p_uft[0]));
}

static long constat(int h)
{
    TYPEAHEAD *bufptr;

    if ((h < 1) || (h > 2))
        return 0;

    bufptr = buffer[h];
    return (bufptr->add > bufptr->remove) ? -1L : Bconstat(h);
}
```

`constat` reports availability for AUX (1) or CON (2) only — for any
other handle (PRN=0, raw=5, redirected=6/7, etc.) it returns 0L
unconditionally.

The typeahead buffer is consulted first — if non-empty, the function
returns `-1L` without polling hardware.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x000B` |
| WORD args | 0 |

## Inputs

None.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |

| Condition                                              | Value  |
|--------------------------------------------------------|--------|
| Typeahead buffer for stdin device non-empty            | `-1L`  |
| Typeahead empty, hardware indicates input available    | `-1L`  |
| Neither                                                | `0L`   |
| stdin redirected to printer/raw/unsupported handle     | `0L`   |

## Errors

None.

## Edge cases

- **Standard-handle redirection** (`p_uft[0] > 0`): dispatcher routes
  via `eof()` on the file descriptor, returning `0L` if EOF and
  `-1L` otherwise.
- **`p_uft[0] = 0` (default = `H_Console = -1` → `HXFORM = 2`)**: normal
  CON behaviour.
- **Constant-time** when typeahead non-empty; otherwise calls into
  BIOS which may briefly disable interrupts.

## Dependencies

- `Bconstat(h)`
- typeahead state, `run->p_uft[0]`

Used by: programs polling for keyboard input, EmuCON, AES event loop
(via `evnt_keybd`).

## Cross-references

- **Compendium:** "Cconis" — agrees.
- **Profibuch:** "Cconis (11)" — agrees.

## Configuration / version

None.

## NewTOS implementation notes

(Empty during Phase 00.)
