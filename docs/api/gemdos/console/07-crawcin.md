# `Crawcin` — GEMDOS 0x07

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xrawcin()`.

## EmuTOS source

- **Primary:** `bdos/console.c:367` — `xrawcin()`
- **Helpers:** `getch()` (l.346)
- **Globals read:** `run->p_uft[0]`, typeahead state
- **Build config:** none.

## Source extraction

```c
long xrawcin(void)
{
    return getch(HXFORM(run->p_uft[0]));
}
```

Reads one character through `getch()` — typeahead-aware, but **without**
the `conbrk()` pre-call that `Cconin`/`Cnecin` perform. So no Ctrl-S
honouring before the read; no Ctrl-C trapping; no echo.

`getch()` blocks via `Bconin()` if the typeahead buffer is empty.
Therefore `Crawcin` is **blocking** — distinct from `Crawio`'s
non-blocking input mode.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0007` |
| WORD args | 0 |

## Inputs

None.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Encoding | scancode (high word) + ASCII byte (low byte). |

## Errors

None.

## Edge cases

- **No Ctrl-C handling**: a Ctrl-C is returned as `0x00000003` to the
  caller; the program is *not* terminated.
- **No Ctrl-S handling**: previously queued Ctrl-S bytes that may sit
  in `con_typeahead` (planted there by some earlier `Cconin`'s
  `conbrk()`) will be returned to the caller as data.
- **Standard-handle redirection** (`p_uft[0] > 0`): dispatcher reads a
  byte from file. (Same as `Cconin`, `Cnecin`.)

## Dependencies

- `Bconin(h)`, typeahead state, `run->p_uft[0]`

Used by: programs implementing custom line discipline that still want
the typeahead buffer (e.g., a shell that wants pre-cooked Ctrl-keys
already buffered by previous calls but no further special processing
on this read).

## Cross-references

- **Compendium:** "Crawcin" — agrees.
- **Profibuch:** "Crawcin (7)" — agrees.

## Configuration / version

None.

## NewTOS implementation notes

(Empty during Phase 00.)
