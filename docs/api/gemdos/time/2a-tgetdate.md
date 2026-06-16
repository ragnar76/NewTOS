# `Tgetdate` — GEMDOS 0x2A

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/time.c:99` (`xgetdate`).

## EmuTOS source

```c
long xgetdate(void) { return current_date; }
```

`current_date` is a `UWORD` global (`bdos/time.c:53`), maintained by
the BIOS clock-tick handler and updated whenever `Settime` (BIOS
function 22) is called. The BDOS function is a pure read of the
cached value — no I/O, no validation.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x002A` |
| WORD args | 0 |
| Stack frame | `+0: 0x002A` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended UWORD packed date) |

### Date encoding (FAT-compatible)

| Bits  | Field  | Range / meaning                       |
|------:|--------|---------------------------------------|
| 15..9 | year   | offset from 1980 (0 ⇒ 1980, 119 ⇒ 2099) |
| 8..5  | month  | 1..12                                 |
| 4..0  | day    | 1..31                                 |

The high 16 bits of the LONG return are zero (the source returns the
UWORD directly, which the C compiler zero-extends to LONG).

## Errors

None — function cannot fail.

## Edge cases

- **No I/O on every call**: the function reads `current_date`
  directly. The cached value is updated by `tikfrk` (file-local,
  invoked from the 200 Hz timer interrupt) when the clock crosses
  midnight, and by `xsetdate` / BIOS `Settime` for explicit
  changes.
- **Y2K and beyond**: bits 15..9 give 128 years from 1980 → 2107.
  EmuTOS rejects years > 119 in `xsetdate` ("warranty expires
  12/31/2099"), but `xgetdate` itself never validates and will
  cheerfully return any value held in `current_date`.
- **Pre-1980 not representable** — the field is unsigned offset.
- **Boot-time value**: BIOS reads RTC (NVRAM clock on Mega ST/STe;
  IKBD clock as fallback). Without RTC, `current_date` defaults
  to a build-time constant (`config.h: OS_CDATE`).

## Dependencies

- `current_date` global (BDOS).
- BIOS `Settime` (function 22) updates it.
- `tikfrk` increments it on midnight rollover.

Used by: `Fsfirst`/`Fsnext` DTA timestamps, `Fcreate` directory
entry timestamping, `Fdatime`, shell `DATE` commands.

## Cross-references

- **Compendium:** "Tgetdate" — agrees.
- **Profibuch:** "Tgetdate (0x2A)" — agrees.
- `docs/api/gemdos/time/2b-tsetdate.md`
- `docs/api/structures/dta.md` — uses same encoding.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07 plan: `current_date` will be driven by an AArch64 generic
timer + an RTC abstraction (RPi4 has no battery-backed RTC by
default; QEMU `aarch64-virt` exposes a PL031). Boot-time default
remains a build constant.
