# `Tgettime` — GEMDOS 0x2C

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/time.c:132` (`xgettime`).

## EmuTOS source

```c
long xgettime(void) { return current_time; }
```

`current_time` is a `UWORD` global (`bdos/time.c:53`), updated by
the BIOS clock-tick handler.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x002C` |
| WORD args | 0 |
| Stack frame | `+0: 0x002C` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended UWORD packed time) |

### Time encoding (FAT-compatible, 2-second resolution)

| Bits  | Field        | Range / meaning                          |
|------:|--------------|------------------------------------------|
| 15..11| hour         | 0..23                                    |
| 10..5 | minute       | 0..59                                    |
| 4..0  | "double-sec" | 0..29 — represents seconds 0..58 in steps of 2 |

The seconds field is `seconds / 2`; odd seconds are not
representable. The high 16 bits of the LONG return are zero.

## Errors

None.

## Edge cases

- **2-second granularity**: programs measuring sub-2-second
  intervals must use BIOS `Tickcal` / 200 Hz timer; `Tgettime`
  cannot resolve odd seconds.
- **Tick coherency with `Tgetdate`**: at midnight, `current_date`
  and `current_time` are updated atomically inside `tikfrk` (timer
  IRQ handler). A program calling `Tgettime` then `Tgetdate` may
  see them out-of-sync only if a context switch happens between
  the two calls — EmuTOS is single-threaded so this cannot occur,
  but NewTOS multitasking will need explicit ordering or a single
  combined accessor.

## Dependencies

- `current_time` global.
- `tikfrk` (timer IRQ).

## Cross-references

- **Compendium:** "Tgettime" — agrees.
- **Profibuch:** "Tgettime (0x2C)" — agrees.
- `docs/api/gemdos/time/2d-tsettime.md`

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: provide an atomic 32-bit fetch (date<<16 | time) for
multitasking-safe reading. The legacy `Tgettime`/`Tgetdate` then
become wrappers that decompose this snapshot.
