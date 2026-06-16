# `Pterm0` — GEMDOS 0x00

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/proc.c:583` (`x0term`).

## EmuTOS source

- **Primary:** `bdos/proc.c:583` — `void x0term(void)`
- **Delegates to:** `xterm(0)` — see `docs/api/gemdos/process/01-pterm.md`

## Source extraction

```c
void x0term(void)
{
    xterm(0);
}
```

`Pterm0` is a thin wrapper that calls `Pterm` with return code zero.
All termination semantics (file close, memory free, parent restore,
user-term handler, return path through `termuser`) are identical to
`Pterm` — only the return code differs.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0000` |
| WORD args | 0 |
| Stack frame | `+0: 0x0000 (fn)` |

## Inputs

None.

## Outputs

**Does not return.** Control transfers to the parent process at the
instruction following the parent's `Pexec` call. Parent's `D0`
receives `0` as the child's exit status (in the low word; high word
is zero for `Pterm0`/`Pterm`, see notes).

## Errors

None — function cannot fail. If the current process has no parent
(boot process), behaviour is the same as for `Pterm`: see that spec
for the "no parent" case (control returns to the BIOS reset path
via `termuser`).

## Edge cases

- **User-term handler at exception 0x102** is invoked first (see
  `Pterm` spec). It runs even for `Pterm0` because the wrapper goes
  through the full `xterm` path.
- **Return value width**: `Pterm0` always exits with `0`. The parent
  reads it as a `LONG` from `D0`. Some Atari documentation states
  that the high word is undefined for `Pterm0`; EmuTOS zeroes it
  explicitly via `run->p_dreg[0] = 0` (LONG store).
- **From a TSR**: a TSR (resident program installed via `Ptermres`)
  cannot meaningfully call `Pterm0` because there is no parent
  context to return to.
- **Standard libraries** rarely call `Pterm0` directly; they typically
  use `Pterm(exit_code)`. `Pterm0` exists for assembly-level "tiny
  exit" code that wants the smallest possible exit stub.

## Dependencies

- `xterm()` — full implementation lives there.

Used by: hand-written assembly programs, the TOS bootloader after
launching the AESPRG completes (rare path).

## Cross-references

- **Compendium:** "Pterm0" — agrees, marked obsolete in favour of
  `Pterm`.
- **Profibuch:** "Pterm0 (0x00)" — agrees.
- **`docs/api/gemdos/process/01-pterm.md`** — full semantics.
- **`docs/api/gemdos/_dispatcher.md`** — `funcs[0]` table entry.

## Configuration

None — present in all builds.

## NewTOS implementation notes

(Empty during Phase 00.)
