# `Cconws` — GEMDOS 0x09

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xconws()`.

## EmuTOS source

- **Primary:** `bdos/console.c:454` — `xconws()`
- **Helpers:** `prt_line()` (l.461), `tabout()` (l.276),
  `conout()` (l.252), `conbrk()`, `Bconout`
- **Globals read:** `run->p_uft[1]`, `glbcolumn[]`
- **Globals written:** `glbcolumn[h]`

## Source extraction

```c
void xconws(char *p)
{
    prt_line(HXFORM(run->p_uft[1]), p);
}

static void prt_line(int h, char *p)
{
    while (*p)
        tabout(h, (unsigned char)*p++);
}
```

Loops calling `tabout` for each byte until a NUL terminator is found.
Tab expansion is performed exactly as in `Cconout`. There is no length
limit — the loop trusts the caller's NUL.

Note the C signature is `void`, but the trap-1 dispatcher unconditionally
returns whatever `xconws()` left in D0. With the GCC m68k ABI compiling
a `void` function, D0 typically holds the address of `p` after the
loop or unspecified scratch. **The TOS API contract is that
`Cconws` has an undefined return value.**

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0009` |
| WORD args | 2 (one LONG = pointer) |
| Stack frame | `+2: p (LONG)`, `+0: 0x0009` |

## Inputs

| Pos | Name | Type    | Notes                                      |
|-----|------|---------|--------------------------------------------|
| 0   | `p`  | char *  | NUL-terminated, in caller's address space  |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Value | undefined (ABI scratch). Atari TOS documentation often shows the value as the number of characters output, but EmuTOS does not implement that. |

## Errors

None.

## Edge cases

- **NUL not found** within accessible memory → wild loop / fault.
  EmuTOS performs no length validation.
- **Tab expansion** identical to `Cconout`'s — see that spec.
- **Ctrl-S in input** during the loop → loop blocks per-character via
  `conbrk()`.
- **Ctrl-C in input** → `terminate()`.
- **Standard-handle redirection** (`p_uft[1] > 0`): dispatcher computes
  `strlen(p)` and writes the entire string in one `xwrite()` call —
  tab expansion is **lost**.

## Dependencies

- `Cconout`'s entire dependency set (`Bconout`, `glbcolumn`, etc.)
- `run->p_uft[1]`

Used by: most C programs (`puts`-equivalent), `EmuCON`, AES messages
on the bottom line.

## Cross-references

- **Compendium:** "Cconws" — describes return as "length printed";
  EmuTOS doesn't honour that. Logged as variant of D-001.
- **Profibuch:** "Cconws (9)" — return value undefined (consistent).

## Configuration / version

None.

## NewTOS implementation notes

(Empty during Phase 00.)
