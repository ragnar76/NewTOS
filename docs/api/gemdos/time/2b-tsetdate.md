# `Tsetdate` — GEMDOS 0x2B

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/time.c:108` (`xsetdate`).

## EmuTOS source

```c
long xsetdate(UWORD d)
{
    UWORD curmo, day;
    const UBYTE *nday = IS_A_LEAP_YEAR(d) ? nday_leap : nday_norm;

    curmo = (d & MTH_BM) >> MTH_SHIFT;     /* MTH_BM=0x01E0, shift=5 */
    day   =  d & DAY_BM;                   /* DAY_BM=0x001F      */

    if (((d >> YRS_SHIFT) > 119)           /* "Warranty expires 12/31/2099" */
     || (curmo > 12)
     || (day > nday[curmo]))
        return ERR;

    Settime(MAKE_ULONG(d, current_time));  /* BIOS fn 22 */
    return E_OK;
}
```

`IS_A_LEAP_YEAR(d) = !(d & 0x0600)` — true when bits 10..9 are both
zero, i.e. year-offset divisible by 4 (1980, 1984, … 2096). Note
this drops the 100/400 Gregorian rules — every 4th year is a leap
year in this encoding. Correct for the 1980..2099 window.

`Settime` updates the BIOS RTC and triggers BDOS to refresh
`current_date` / `current_time`.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x002B` |
| WORD args | 1 (UWORD) |
| Stack frame | `+2: d (WORD)`, `+0: 0x002B` |

## Inputs

| Pos | Name | Type  | Range / meaning                       |
|-----|------|-------|---------------------------------------|
| 0   | `d`  | UWORD | packed date (same encoding as Tgetdate) |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (GEMDOS error code) |

| Case                                | Return       |
|-------------------------------------|--------------|
| Date accepted, RTC updated          | `E_OK` (0)  |
| Year > 119 (post-2099)              | `ERR` (-1)  |
| Month == 0 or month > 12            | `ERR` (-1)  |
| Day == 0 or day > days-in-month     | `ERR` (-1)  |

## Errors

`ERR` (-1) — the only error. Note this is **not** `EBADRQ` /
`EINVFN` etc., it is literal -1 from `bdos/gemerror.h`.

## Edge cases

### 1. Day == 0 rejection

`day > nday[curmo]` rejects day 0 implicitly because `nday[m] >= 28`
for all valid months and the comparison is unsigned: day 0 is **not**
rejected by this single check — but `day = d & 0x1F`; if `d & 0x1F == 0`
then day == 0 and `0 > nday[curmo]` is false. **EmuTOS accepts day 0**.
Documented Atari behaviour: classic TOS rejects day 0; EmuTOS does
not. Discrepancy candidate (see D-007 below).

### 2. Month == 0

Same issue — month 0 is not rejected by `curmo > 12` either. The
`nday[0] = 0` entry then makes `day > 0` reject any day except 0.
So `(month=0, day=0)` is **accepted**, setting an invalid date.

### 3. Leap year shortcut

The leap-year rule `!(d & 0x0600)` works because `(year_offset & 3) == 0`
iff bits 1..0 of year are zero, which after the 9-bit shift land in
bits 10..9 of the date word. Year-offset 100 (= 2080) is **correctly**
flagged as leap by this rule, even though Gregorian says century
years aren't leap unless divisible by 400 — but 2080 is divisible
by 4, not 100, so it *is* a leap year. The 2100/2200/2300 case
(non-leap) would matter — but year-offset 120+ is rejected anyway.

### 4. Year 0 (1980) accepted

Year offset 0 is valid (= 1980). The check is `> 119`.

### 5. RTC write atomicity

`Settime` packs the new date with the *current* time
(`MAKE_ULONG(d, current_time)`). If a tick interrupt updates
`current_time` between the read and the BIOS call, there is a
single-tick race window. EmuTOS does not guard against this; the
race is benign because the resulting time is at most one tick off.

### 6. Date stored as packed UWORD

The BDOS does not normalise / validate further — a date passed
`Tsetdate` survives unchanged through the BIOS round-trip.

## Discrepancy candidate

**D-007**: `xsetdate` accepts day 0 and month 0 due to insufficient
range checks. Compendium states "valid date required". NewTOS
should add `day >= 1 && curmo >= 1` checks. (Append to
`_discrepancies.md` if confirmed via Atari TOS hardware run.)

## Dependencies

- BIOS `Settime` (fn 22).
- `current_time` global (read for repacking).
- `nday_norm` / `nday_leap` tables.

Used by: shell `DATE` command, AES control panel, RTC-resync
utilities.

## Cross-references

- **Compendium:** "Tsetdate" — agrees on encoding & error.
- **Profibuch:** "Tsetdate (0x2B)" — agrees.
- `docs/api/gemdos/time/2a-tgetdate.md`

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

Apply the day≥1 / month≥1 fix during PHASE-07. ABI-visible: programs
that incidentally relied on `(0,0)` succeeding (none known) will
see `ERR`. Acceptable given Compendium semantics.
