# `Cconrs` — GEMDOS 0x0A

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xconrs()`.

## EmuTOS source

- **Primary:** `bdos/console.c:519` — `xconrs()`
- **Helpers:** `cgets()` (l.527), `getch()`, `conout()`, `cookdout()`,
  `tabout()`, `backsp()` (l.484), `newline()` (l.471), `terminate()`
- **Globals read:** `run->p_uft[0]`, `run->p_uft[1]`, `glbcolumn[]`
- **Globals written:** `glbcolumn[h]`

## Source extraction

```c
void xconrs(char *p)
{
    p[1] = cgets(HXFORM(run->p_uft[0]),
                 (unsigned char)p[0],
                 &p[2]);
}
```

The argument `p` points at a *line-input record*:

```
p[0]  IN:  max length (0–255)        — caller-supplied limit
p[1]  OUT: actual length             — written by Cconrs
p[2..]: actual character buffer      — caller-allocated
```

`cgets(h, maxlen, buf)` (l.530) implements a small line editor on the
device referenced by `h`:

| Key                       | Action                                                     |
|---------------------------|------------------------------------------------------------|
| CR / LF                   | echo CR, return current `retlen`.                          |
| BS (0x08) / RUB (0x7F)    | back over one cooked-out character.                        |
| Ctrl-C (0x03)             | `terminate()` — process exits.                             |
| Ctrl-X (0x18)             | erase entire line (repeated backspace).                    |
| Ctrl-U (0x15)             | print `'#'`, newline, restart input.                       |
| Ctrl-R (0x12)             | print `'#'`, newline, redisplay current buffer; continue.  |
| any other byte            | store in `buf[retlen++]`, echo via `cookdout()`.           |

`cookdout()` (l.310) prints `^X` for control chars 0x00–0x1F (preserving
`\t`'s tab expansion), and prints printable bytes verbatim.

`backsp()` (l.484) recomputes the column the cursor would be at after
removing the last character, then emits `BS SP BS` until the cursor
lands there. It uses `glbcolumn[h]` and the contents of `buf` to do
this — i.e., it accounts for `^X` width-2 control echo and tab
expansion.

The loop terminates when:
- The user enters CR or LF, **or**
- `retlen == maxlen`.

`maxlen == 0` makes the loop never execute; `cgets` returns 0
immediately. Output buffer is *not* NUL-terminated — TOS contract is
length-prefixed.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x000A` |
| WORD args | 2 (one LONG = pointer) |
| Stack frame | `+2: p (LONG)`, `+0: 0x000A` |

## Inputs

| Pos | Name | Type   | Notes                                          |
|-----|------|--------|------------------------------------------------|
| 0   | `p`  | char * | line-input record: `[max, len, buf...]`        |

## Outputs

`p[1]` is set to the number of characters written into `p[2..]`.

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Value | undefined (`xconrs` returns `void`; D0 holds ABI scratch). Atari documentation calls this "Cconrs returns no value". |

## Errors

None at the function level. Ctrl-C terminates the process.

## Edge cases

- **`maxlen == 0`** → returns immediately; `p[1] = 0`.
- **No NUL terminator written** — caller must use `p[1]` as length.
- **Buffer overrun** is impossible by construction: the inner loop
  bounds `retlen` by `maxlen` before storing, and BS / Ctrl-X only
  decrement `retlen` (never below 0).
- **Tab in buffer**: stored verbatim as `\t`; redisplay (`Ctrl-R`)
  re-expands via `cookdout()`.
- **Echo target = stdin's device** (`run->p_uft[0]`), NOT stdout
  (`p_uft[1]`). The `xconrs` wrapper passes `HXFORM(run->p_uft[0])` for
  *both* read and echo.
- **Ctrl-S during entry** is caught by `conbrk()` inside the cycle of
  `conout()` calls during echo, blocking input until Ctrl-Q.
- **CR is echoed as CR only** — no LF is emitted. Applications that
  want a clean newline must `Cconout(LF)` themselves after `Cconrs`.
- **Standard-handle redirection** (`p_uft[0] > 0`): dispatcher
  implements its own simplified read-line loop directly against
  `xread()`. The line-editor keys (BS, Ctrl-U, etc.) are not
  interpreted in that path. See `docs/api/gemdos/_dispatcher.md` §5.

## Dependencies

- `Bconin`, `Bconout`, `Bconstat`
- `Pterm` (via `terminate()`)
- `run->p_uft[0]`, `glbcolumn`
- typeahead state

Used by: BASIC `INPUT`, command-line interpreters, configuration
prompts in many TOS programs.

## Cross-references

- **Compendium:** "Cconrs" — agrees on signature.
- **Profibuch:** "Cconrs (10)" — agrees, including buffer layout.

## Configuration / version

- `CONF_WITH_VT52`: affects how `cookdout` rendering looks but not
  semantics.

## NewTOS implementation notes

(Empty during Phase 00. **Blocked on `Pterm`** — implement the leaf
console functions first; defer this until process management has at
least the `xterm(-32)` path.)
