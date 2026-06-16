# `Cconin` — GEMDOS 0x01

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xconin()`.

## EmuTOS source

- **Primary:** `bdos/console.c:386` — `xconin()`
- **Helpers:** `conbrk()` (line 188), `getch()` (line 346), `conin()` (line 376),
  `Bconin` (BIOS 0x02), `Bconout` (BIOS 0x03), `terminate()` (= `xterm(-32)`)
- **Headers:** `bdos/console.h` (`H_Console`, `HXFORM`), `bdos/proc.h`,
  `include/biosbind.h`
- **Globals read:**  `run->p_uft[0]` (stdin standard handle), `con_typeahead`
  buffer, `glbcolumn[]`
- **Globals written:** `con_typeahead.add/remove`, `glbcolumn[h]`
- **Build config:** none directly (transitive: VT52 via `Bconout`)

## Source extraction

```c
long xconin(void)
{
    int  h  = HXFORM(run->p_uft[0]);   /* stdin -> BIOS device 2 normally */
    conbrk(h);                         /* drain Bconstat into typeahead;
                                          honour Ctrl-S, Ctrl-X, Ctrl-Q;
                                          Ctrl-C in this drain → terminate */
    long ch = conin(h);                /* getch() then echo via conout() */
    if (LOBYTE(ch) == 0x03) terminate();
    return ch;
}
```

`conin()` (l.376) reads one char via `getch()` (l.346) — which prefers the
typeahead buffer over `Bconin` — then echoes the character via `conout()`,
which itself first invokes `conbrk()` to honour any Ctrl-S/Ctrl-X/Ctrl-Q
the user may have queued, then calls `Bconout` and updates `glbcolumn[h]`.

`getch()` returns the BIOS scancode-+-ASCII LONG verbatim; the high word
contains the IKBD scancode, the low word the ASCII byte.

`Cconin` is **blocking** — `Bconin` blocks until input is available.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0001` |
| WORD args | 0 |
| Stack frame on entry | `+0: 0x0001 (fn)` |

## Inputs

None. (`run->p_uft[0]` is implicit per-process state, not a parameter.)

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |
| Encoding | High word: IKBD scancode. Low word: high byte = 0, low byte = ASCII. |

Echoed character is also written to the device in `run->p_uft[1]` indirectly,
because `conin()` invokes `conout()` on the same handle `h` (= stdin device).

## Errors

None defined. Always returns the read character; cannot fail under normal
operation. Hardware-detected errors are absorbed by the BIOS layer.

## Edge cases

- **Ctrl-C in typeahead drain:** if `conbrk()` (called before `conin()`)
  observes a Ctrl-C, it invokes `terminate()` directly — `Cconin` does not
  return.
- **Ctrl-C as fresh input:** the secondary `if (LOBYTE(ch) == ctrlc) terminate()`
  catches a Ctrl-C that arrived through the regular `getch()` path.
- **Ctrl-S queued before call:** `conbrk()` sets a local `stop` flag and
  spins until Ctrl-Q arrives; `Cconin` blocks inside `conbrk()` for the
  duration. No bytes are lost — the loop re-buffers all non-Ctrl-S bytes
  into `con_typeahead`.
- **Standard-handle redirection** (`run->p_uft[0] > 0`): the GEMDOS dispatcher
  intercepts the call and reads one byte from the file via `xread()` instead;
  this function body is **not** entered. See `docs/api/gemdos/_dispatcher.md`
  §5. Discrepancy `D-004`.
- **Echoed character placement:** if `run->p_uft[1]` was redirected (e.g.,
  stdout to file), the dispatcher does *not* intercept echo here — the echo
  is performed by `conout(h)` on the *stdin* handle `h`, not the stdout
  handle. So redirected-stdout still echoes to the terminal.

## Dependencies

- `Bconin(h)`, `Bconstat(h)`, `Bconout(h, c)`
- `Pterm` (via `terminate()`) — required for Ctrl-C handling
- typeahead buffer state (`con_typeahead`)
- `glbcolumn[h]`
- per-process `run->p_uft[0]`

Used by: programs (single-character read), C-library `getchar`.

## Cross-references

- **Compendium:** "Cconin" — agrees on signature and blocking semantics.
- **Profibuch:** "Cconin (1)" — same.
- **`docs/api/gemdos/_dispatcher.md`** §5 — redirection routing.
- **`docs/api/_dependency-graph.md`** — Cconin entry.

## Configuration / version

- `CONF_WITH_VT52`: affects echo rendering only, not function semantics.
- `CONF_SERIAL_CONSOLE`: changes the underlying BIOS device but not the
  GEMDOS-level behaviour of `Cconin`.

## NewTOS implementation notes

(Empty during Phase 00. Filled in during Phase 02.)
