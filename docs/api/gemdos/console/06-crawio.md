# `Crawio` — GEMDOS 0x06

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xrawio()`.

## EmuTOS source

- **Primary:** `bdos/console.c:434` — `xrawio()`
- **Helpers:** `constat()` (l.144), `getch()` (l.346), `Bconout`
- **Globals read:** `run->p_uft[0]`, `run->p_uft[1]`, typeahead
- **Build config:** none directly.

## Source extraction

```c
long xrawio(int parm)
{
    int i;

    if (parm == 0xFF)
    {
        i = HXFORM(run->p_uft[0]);
        return constat(i) ? getch(i) : 0L;
    }

    Bconout(HXFORM(run->p_uft[1]), parm);
    return 0;                          /* "dummy" — comment in source */
}
```

Behaviour bifurcates on `parm`:

- `parm == 0xFF` → **input mode**: poll input device (stdin); if a
  character is available, return it; **otherwise return 0L**
  immediately (non-blocking). No Ctrl-C, Ctrl-S, Ctrl-X handling.
- `parm != 0xFF` → **output mode**: write `parm` to the output device
  (stdout) directly via `Bconout`; no tab expansion, no Ctrl-S pause,
  no `glbcolumn` update. Returns 0.

See discrepancy `D-002`: the non-blocking nature of input mode is a
common point of confusion.

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0006` |
| WORD args | 1 |
| Stack frame | `+2: parm (WORD)`, `+0: 0x0006` |

## Inputs

| Pos | Name   | Type | Notes                                       |
|-----|--------|------|---------------------------------------------|
| 0   | `parm` | WORD | `0x00FF` ⇒ read; any other value ⇒ write byte |

EmuTOS compares the *full* WORD against `0xFF` (i.e., `0x00FF`).
Passing `0x80FF` would *not* select read mode — it is treated as a
write of byte `0xFF`. (Atari TOS sign-extended the parameter; EmuTOS
narrows.)

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |

| Mode    | Value                                                        |
|---------|--------------------------------------------------------------|
| Read    | typeahead/Bconin LONG, **or** `0L` if no character available |
| Write   | always `0`                                                   |

## Errors

None.

## Edge cases

- **Read-mode return value `0`** is ambiguous with a successful read of
  the NUL byte. Applications using `Crawio` for input must use
  `constat()` directly via `Cconis` (0x0B) if they need to distinguish.
- **Ctrl-C in `getch()`** is NOT trapped here — the byte is returned
  to the caller. `Crawio` is genuinely raw: even ETX passes through.
- **Output mode bypasses tab expansion**: writing `'\t'` emits the
  literal byte 0x09, not 8 spaces.
- **Output mode bypasses `conbrk()`**: pending Ctrl-S/Ctrl-X queued by
  the user has no effect on `Crawio` output.
- **Typeahead buffer interaction**: read mode goes through `getch()`,
  which prefers the typeahead buffer. Bytes queued by an earlier
  `Cconin`/`Cnecin`'s `conbrk()` will be returned.
- **Standard-handle redirection** (`p_uft[0] > 0` or `p_uft[1] > 0`):
  dispatcher intercepts; the `0xFF` parameter selects file read of one
  byte (returns `EOF_INDICATOR` if EOF), other values select a
  one-byte file write.

## Dependencies

- `Bconout(h, c)`, `Bconin(h)`, `Bconstat(h)`, `constat()`, `getch()`
- `run->p_uft[0]`, `run->p_uft[1]`
- typeahead state (read shares with `Cconin`/`Cnecin`)

Used by: programs implementing their own line discipline (e.g.,
shells, terminal emulators) that need un-cooked I/O.

## Cross-references

- **Compendium:** "Crawio" — sometimes incorrectly stated as blocking
  in input mode. EmuTOS is non-blocking; this is `D-002`.
- **Profibuch:** "Crawio (6)" — correctly describes non-blocking.
- **`docs/api/_discrepancies.md` D-002**.

## Configuration / version

None.

## NewTOS implementation notes

(Empty during Phase 00.)
