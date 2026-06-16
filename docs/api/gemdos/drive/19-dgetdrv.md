# `Dgetdrv` — GEMDOS 0x19

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsmain.c:438` (`xgetdrv`).

## EmuTOS source

```c
long xgetdrv(void) { return run->p_curdrv; }
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0019` |
| WORD args | 0 |
| Stack frame | `+0: 0x0019` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG |

Current drive of the calling process: 0 ⇒ A:, …, 15 ⇒ P:. Always
within `0..BLKDEVNUM-1` because `xsetdrv` clamps. High word zero.

## Errors

None.

## Edge cases

- `p_curdrv` is a single byte in the PD (`p_uft[]` neighbour); the
  function returns its zero-extended value.
- Per-process — different children may report different current
  drives.

## Dependencies

- `run` PD pointer.

## Cross-references

- **Compendium:** "Dgetdrv" — agrees.
- **Profibuch:** "Dgetdrv (0x19)" — agrees.
- `docs/api/gemdos/drive/0e-dsetdrv.md`
- `docs/api/structures/basepage.md`

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)
