# `Tsettime` — GEMDOS 0x2D

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/time.c:142` (`xsettime`).

## EmuTOS source

```c
long xsettime(UWORD t)
{
    if (((t & SEC_BM) >= 30)                  /* SEC_BM=0x001F */
     || ((t & MIN_BM) >= (60 << MIN_SHIFT))   /* MIN_BM=0x07E0 */
     || ((t & HRS_BM) >= (24 << HRS_SHIFT)))  /* HRS_BM=0xF800 */
        return ERR;

    Settime(MAKE_ULONG(current_date, t));     /* BIOS fn 22 */
    return E_OK;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x002D` |
| WORD args | 1 (UWORD) |
| Stack frame | `+2: t (WORD)`, `+0: 0x002D` |

## Inputs

| Pos | Name | Type  | Range / meaning                            |
|-----|------|-------|--------------------------------------------|
| 0   | `t`  | UWORD | packed time (same encoding as Tgettime)    |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (GEMDOS error code) |

| Case                                  | Return     |
|---------------------------------------|------------|
| Time accepted                          | `E_OK` (0)|
| Double-second field ≥ 30               | `ERR` (-1)|
| Minute field ≥ 60                      | `ERR` (-1)|
| Hour field ≥ 24                        | `ERR` (-1)|

## Errors

`ERR` (-1) — single error code.

## Edge cases

### 1. Field validation, not value validation

The check `(t & SEC_BM) >= 30` validates the **double-second**
field directly. `seconds = (t & 0x1F) * 2`, so the maximum
representable seconds = 58. Any encoded value 30..31 is rejected
because it would represent ≥ 60 seconds.

### 2. Minute / hour ranges

`60 << MIN_SHIFT = 0x780`, so any minute field 60..63 is rejected.
`24 << HRS_SHIFT = 0xC000`, hours 24..31 rejected.

### 3. RTC write packs current date

Like `Tsetdate`, the call repacks with `current_date` to keep the
RTC's other half intact. Same single-tick race window as
`Tsetdate`.

### 4. No "set sub-second precision" path

`Tsettime` cannot set odd seconds. Programs that need accurate
"set to NTP time" must accept ≤2 s drift.

## Dependencies

- BIOS `Settime` (fn 22).
- `current_date` global (read for repacking).

## Cross-references

- **Compendium:** "Tsettime" — agrees.
- **Profibuch:** "Tsettime (0x2D)" — agrees.
- `docs/api/gemdos/time/2c-tgettime.md`

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)
