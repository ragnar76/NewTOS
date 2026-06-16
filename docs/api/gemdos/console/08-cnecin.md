# `Cnecin` — GEMDOS 0x08

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` — `bdos/console.c:xnecin()`.

## EmuTOS source

- **Primary:** `bdos/console.c:411` — `xnecin()`
- **Helpers:** `conbrk()` (l.188), `getch()` (l.346), `terminate()`
- **Globals read:** `run->p_uft[0]`, typeahead state
- **Globals written:** typeahead state (via `conbrk()`)

## Source extraction

```c
long xnecin(void)
{
    int  h  = HXFORM(run->p_uft[0]);
    long ch;

    conbrk(h);                         /* drain Bconstat → typeahead;
                                          honour Ctrl-S/X/Q;
                                          Ctrl-C in this drain → terminate */
    ch = getch(h);                     /* no echo, unlike conin() */
    if (LOBYTE(ch) == 0x03) terminate();
    return ch;
}
```

Identical to `xconin()` except for the absence of an echo step. Echo
is omitted because `xnecin` calls `getch()` directly rather than
`conin()` (which calls `getch()` *and* `conout()`).

## Trap interface

| | |
|--|--|
| Trap | 1 |
| Function ID | `0x0008` |
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

None. Ctrl-C terminates the process rather than returning.

## Edge cases

- **Ctrl-C** in `conbrk()` drain *or* fresh in `getch()` → process
  terminates (no return).
- **Ctrl-S** before the call → blocks inside `conbrk()` until Ctrl-Q.
- **Standard-handle redirection** (`p_uft[0] > 0`): dispatcher reads
  one byte from file; this body not entered.
- **Typical use**: password input, single-key prompts.

## Dependencies

- `Bconin`, `Bconstat`, `Bconout` (latter for Ctrl-G "buffer-full" beep)
- `Pterm` (via `terminate()`)
- typeahead, `run->p_uft[0]`

Used by: `cgets()` (the `Cconrs` line editor) reads through this path
internally (actually `cgets` calls `getch()` directly, not `xnecin`,
but the semantics are equivalent).

## Cross-references

- **Compendium:** "Cnecin" — agrees.
- **Profibuch:** "Cnecin (8)" — agrees.

## Configuration / version

None.

## NewTOS implementation notes

(Empty during Phase 00.)
