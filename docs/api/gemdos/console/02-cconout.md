# `Cconout` — GEMDOS 0x02

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xconout()`.

## EmuTOS source

- **Primary:** `bdos/console.c:268` — `xconout()`
- **Helpers:** `tabout()` (l.276), `conout()` (l.252), `conbrk()` (l.188),
  `Bconout` (BIOS 0x03)
- **Globals read:** `run->p_uft[1]` (stdout standard handle), `glbcolumn[]`
- **Globals written:** `glbcolumn[h]` (per-device column for tab expansion)
- **Build config:** transitive only (VT52 via `Bconout`)

## Source extraction

```c
long xconout(int ch)
{
    tabout(HXFORM(run->p_uft[1]), ch);
    return 1;                                  /* see D-001 */
}
```

`tabout(h, ch)` (l.276): if `ch == '\t'` (0x09), emit spaces via `conout`
until `glbcolumn[h] & 7 == 0`; otherwise `conout(h, ch)`.

`conout(h, ch)` (l.252):
1. `conbrk(h)` — process any queued Ctrl-S/Ctrl-X/Ctrl-C from input;
   may **block** spinning on Ctrl-S until Ctrl-Q.
2. `Bconout(h, ch)` — write the byte.
3. Update `glbcolumn[h]`: increment for printable (`>= ' '`); reset to 0
   for CR (`0x0D`); decrement for BS (`0x08`); leave alone otherwise.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0002` |
| WORD args | 1 |
| Stack frame | `+2: ch (WORD)`, `+0: 0x0002 (fn)` |

## Inputs

| Pos | Name | Type | Notes                                   |
|-----|------|------|-----------------------------------------|
| 0   | `ch` | WORD | only low 8 bits used; high byte ignored |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Value | always `1` |

See discrepancy `D-001`: documentation often claims return value 0 or
the function code; EmuTOS unambiguously returns `1`.

## Errors

None.

## Edge cases

- **Tab expansion** uses `glbcolumn[h]` which is per-device. Two
  applications writing to the same device share the column counter. The
  counter is reset on `\r`, decremented on `\b`, otherwise tracks
  printable runs only — control characters other than CR/BS leave it
  unchanged, which can desynchronise it from the actual cursor.
- **Embedded Ctrl-S in input** while `Cconout` is active: `conbrk()`
  inside `conout()` will make the function block until the user presses
  Ctrl-Q. This is the documented pause-on-output behaviour.
- **Ctrl-C in input** while `Cconout` is active → `conbrk()` calls
  `terminate()` (= `xterm(-32)`) → the function does not return.
- **Standard-handle redirection** (`run->p_uft[1] > 0`): dispatcher
  intercepts; `tabout` is not invoked. Tab expansion is **lost** when
  stdout is redirected to a file — the raw byte (including `0x09`) is
  written verbatim. This matches Atari TOS.
- **Upper byte of `ch`** is silently discarded.

## Dependencies

- `Bconout(h, c)`
- `glbcolumn[h]`
- `run->p_uft[1]`

Used by: `Cconws`, `Cconrs` (echo), C-library `putchar`/`puts`/`printf`.

## Cross-references

- **Compendium:** "Cconout" — return value undocumented / inconsistent.
- **Profibuch:** "Cconout (2)" — return value undefined.
- **`docs/api/_discrepancies.md` D-001** — return value `1`.
- **`docs/api/gemdos/_dispatcher.md` §5** — redirection.

## Configuration / version

- `CONF_WITH_VT52`: enables VT52 escape interpretation in `Bconout`'s
  screen handler. Does not change `Cconout`'s semantics at the GEMDOS
  level.

## NewTOS implementation notes

(Empty during Phase 00. Filled in during Phase 02.)
