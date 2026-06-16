# `Cauxis` — GEMDOS 0x12

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xauxistat()`.

## EmuTOS source

- **Primary:** `bdos/console.c:180` — `xauxistat()`
- **Helpers:** `constat()` (l.144), `Bconstat`
- **Globals read:** `run->p_uft[2]`, `aux_typeahead`

## Source extraction

```c
long xauxistat(void)
{
    return constat(HXFORM(run->p_uft[2]));
}
```

Goes through the same `constat()` as `Cconis`. For default
configuration (`p_uft[2] = H_Aux = -2 → HXFORM = 1`), `constat` falls
into its `(h<1)||(h>2)` test → `h == 1`, accepted; consults
`aux_typeahead.add > aux_typeahead.remove`. **However, no GEMDOS
function ever populates `aux_typeahead`** (see `D-003`), so the
typeahead branch is dead in current EmuTOS — the function falls
through to `Bconstat(1)`.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0012` |
| WORD args | 0 |

## Inputs

None.

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Value | `-1L` if a byte is available on AUX (or queued in unused typeahead), else `0L`. |

## Errors

None.

## Edge cases

- **Standard-handle redirection** (`p_uft[2] > 0`): dispatcher uses
  `eof()` over the file.
- **AUX redirected to non-AUX device** via `p_uft[2]` set to
  e.g. `H_Console`: `constat` is still called with the corresponding
  handle (2 = CON) → reports CON typeahead status, not AUX. This is a
  subtle but documented effect of the standard-handle abstraction.

## Dependencies

- `Bconstat(h)`, `run->p_uft[2]`

Used by: programs polling RS-232.

## Cross-references

- **Compendium:** "Cauxis" — agrees.
- **Profibuch:** "Cauxis (18)" — agrees.
- **`docs/api/_discrepancies.md` D-003**.

## Configuration / version

- AUX BIOS device: `CONF_WITH_MFP`, `CONF_WITH_SCC`.

## NewTOS implementation notes

(Empty during Phase 00.)
