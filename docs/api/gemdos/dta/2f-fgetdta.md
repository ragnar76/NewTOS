# `Fgetdta` — GEMDOS 0x2F

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsmain.c:394` (`xgetdta`).

## EmuTOS source

```c
DTAINFO *xgetdta(void) { return (DTAINFO *)run->p_xdta; }
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x002F` |
| WORD args | 0 |
| Stack frame | `+0: 0x002F` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (DTA pointer) |

Current `p_xdta` of the calling process, cast to LONG.

## Errors

None.

## Edge cases

- **Default value**: if the process never called `Fsetdta`, returns
  `&p_cmdlin` (offset 0x80 in the basepage) — see
  `docs/api/structures/basepage.md`. Programs that haven't touched
  the DTA and then call `Fsfirst` overwrite their command-line.
  This is **Atari ABI** — programs depend on it.

## Dependencies

- `run` PD pointer.

## Cross-references

- **Compendium:** "Fgetdta" — agrees.
- **Profibuch:** "Fgetdta (0x2F)" — agrees.
- `docs/api/gemdos/dta/1a-fsetdta.md`
- `docs/api/structures/dta.md`
- `docs/api/structures/basepage.md`

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)
