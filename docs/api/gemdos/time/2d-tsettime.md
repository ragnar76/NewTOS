# `Tsettime` (GEMDOS 0x2D)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/time.c:142` (`xsettime`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the packed-time WORD input encoding, the field
> range validation and the single `ERR` (-1) error, that a valid set both
> updates the GEMDOS time and persists to the RTC. Reference: the XBIOS /
> `settime()` internals.

## EmuTOS source

**Primary location:** `bdos/time.c:142` — `long xsettime(UWORD t)`. Validates
the packed time's fields, then forwards to XBIOS `Settime`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x2D` =
`xsettime`, `stdio_typ = 0`, `wparms = 1` (`bdos/bdosmain.c:235`).

**Helpers called:**
- `include/xbiosbind.h:40` — `Settime(a)` = `xbios_v_l(22, a)` — **XBIOS
  function 22 (0x16)**, not a BIOS call. Reaches `settime()`
  (`bios/clock.c:1218`).
- `bios/clock.c:1218` — `settime(LONG time)` — writes `current_time`/
  `current_date` from the packed LONG, then persists to the machine's RTC.

**Headers / type definitions used:**
- `bdos/time.c:58-71` — time field masks/shifts (`SEC_BM=0x1F`,
  `MIN_BM=0x7E0`, `HRS_BM=0xF800`, `MIN_SHIFT=5`, `HRS_SHIFT=11`).
- `include/portab.h:139` — `MAKE_ULONG(hi, lo)`.
- `include/gemerror.h` — `ERR` (-1), `E_OK` (0).

**System variables / globals read:**
- `current_date` (`bdos/time.c:53`) — repacked with the new time so the RTC's
  date half is preserved.

**System variables / globals written (via `settime()`):**
- `current_date`, `current_time` (`bios/clock.c:1221-1222`).

**Build configuration affecting this function:** `xsettime` itself is
config-independent. Which RTC `settime()` persists to depends on the
`CONF_WITH_*RTC*` / `CONF_WITH_IKBD_CLOCK` set; the Atari standard/default path
is the IKBD clock (`isetdt`, `bios/clock.c:1067`).

---

## Source extraction summary

`Tsettime` sets the system time from a FAT/DOS-packed WORD (2-second
resolution). It validates **each field against its range** directly on the
packed word: the seconds ("double-seconds") field must be < 30 (else it would
mean ≥ 60 s), the minutes field < 60, the hours field < 24. Any out-of-range
field returns `ERR` (-1); otherwise it calls XBIOS `Settime` with the new time
packed against the **current** date (`MAKE_ULONG(current_date, t)`).

At the XBIOS level, `settime()` (`bios/clock.c:1218`) updates the GEMDOS
globals `current_time`/`current_date` directly and writes the value to the RTC
hardware — so a successful `Tsettime` **persists** (durable across the next
boot's RTC read). On the standard Atari path with no add-on RTC, that is the
IKBD clock.

The validation compares the *masked field still in place* against the field
value shifted up: e.g. `(t & MIN_BM) >= (60 << MIN_SHIFT)` rejects minute
fields 60-63 without extracting them. There is no lower-bound problem here (all
fields are 0-based and 0 is valid), so — unlike `Tsetdate` — `Tsettime` has no
day-0/month-0-style gap.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x2D
- **Stack frame on entry (m68k):**
  ```
  +2: t  (word)  — packed time
   0: function_id (word) = 0x002D
  ```
- **Total parameter bytes:** 4 (ID + WORD) → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x2D` in `x8`; the packed-time WORD in `x0`;
  LONG result (`E_OK` or `ERR`) **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `t`  | `UWORD` (16) — packed time |

---

## Inputs

| Position | Name | Type    | Size | Source location   | Notes |
|----------|------|---------|------|-------------------|-------|
| arg0     | `t`  | `UWORD` | 2    | `bdos/time.c:142` | packed time (same encoding as `Tgettime`) |

### Packed time WORD encoding

| Bits   | Field         | Valid field values | Rejected when                          |
|-------:|---------------|--------------------|----------------------------------------|
| 15-11  | hour          | 0-23               | `(t & 0xF800) >= (24 << 11)` i.e. hour ≥ 24 |
| 10-5   | minute        | 0-59               | `(t & 0x07E0) >= (60 << 5)` i.e. min ≥ 60   |
| 4-0    | "double-secs" | 0-29               | `(t & 0x001F) >= 30` i.e. ≥ 60 seconds  |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (GEMDOS error code)
- **Encoding:** `E_OK` (0) on success; `ERR` (-1) on any field failure.
- **Source location:** `bdos/time.c:147` (`ERR`), `bdos/time.c:152` (`E_OK`).

| Case                                 | Return       |
|--------------------------------------|--------------|
| Time accepted, GEMDOS + RTC updated  | `E_OK` (0)   |
| Double-second field ≥ 30             | `ERR` (-1)   |
| Minute field ≥ 60                    | `ERR` (-1)   |
| Hour field ≥ 24                      | `ERR` (-1)   |

### Side effects on global state

- On success, `current_time` and `current_date` are updated
  (`bios/clock.c:1221-1222`, via `Settime`).

### Side effects on hardware

- On success, the new time is written to the machine's RTC. On a failed field
  check the function returns before `Settime`, so **no** hardware or global
  state changes.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Double-second field ≥ 30 (≥ 60 seconds) | `bdos/time.c:144` | `ERR` (-1) |
| Minute field ≥ 60 | `bdos/time.c:145` | `ERR` (-1) |
| Hour field ≥ 24 | `bdos/time.c:146` | `ERR` (-1) |

`ERR` (-1) is the **only** error — the literal generic error from
`include/gemerror.h`, not `EBADRQ`/`EINVFN`.

**Error paths not in Compendium:** none extra.

---

## Verification criteria

- **Nominal:** `Tsettime(0x73BA)` (hour 14, minute 30, double-seconds 26 →
  14:30:52) returns `E_OK` (0); afterwards `Tgettime()` returns `0x000073BA`
  and the RTC holds the new time.
- **Odd second dropped:** to set 14:30:53 a caller must pass double-seconds
  `53/2 = 26`, so `Tgettime` reads back 14:30:52 — the encoding cannot carry
  the odd second (not an error, a resolution limit).
- **Seconds overflow rejected:** double-second field 30 or 31 → `ERR` (-1),
  no state change.
- **Minute 60 rejected:** minute field 60 → `ERR` (-1).
- **Hour 24 rejected:** hour field 24 → `ERR` (-1).
- **Boundary accepted:** 23:59:58 (`(23<<11)|(59<<5)|29 = 0xBFBD`) returns
  `E_OK`.

---

## Edge cases discovered in source

1. **Field-range validation, not value validation.** The checks test the
   masked field directly against its ceiling (`SEC_BM >= 30`,
   `MIN_BM >= 60<<5`, `HRS_BM >= 24<<11`), so seconds 58 (double-sec 29) is the
   max accepted (`bdos/time.c:144-146`). All lower bounds are 0, which is
   valid, so there is no `Tsetdate`-style day-0 gap.

2. **Cannot set odd seconds.** `Tsettime` accepts only even seconds (double
   -seconds × 2). A caller wanting "exact" time accepts ≤ 2 s quantisation.

3. **Persists to the RTC, not just RAM.** `Settime` → `settime()` writes the
   time to the real clock (`bios/clock.c:1228-1263`); durable across boot.

4. **Repacks with the *current* date — one-tick race.** `MAKE_ULONG(
   current_date, t)` reads `current_date` after validation; a midnight tick
   between the read and the `Settime` call could pair the new time with the
   pre-rollover date. EmuTOS does not guard this benign window.

---

## Dependencies

### Requires (must be initialized / functional)

- XBIOS `Settime` (22 / 0x16) → `settime()` → an RTC path (or the IKBD-clock
  fallback).
- `current_date` global (read for repacking).

### Used by

- Shell `TIME` command, AES control panel, RTC-resync utilities.

---

## Related data structures

- `docs/api/gemdos/_state.md` §12 — `current_date`/`current_time` globals + RTC
  path.
- `docs/api/gemdos/time/2c-tgettime.md` — the getter (same encoding).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: "sets the
  time; returns 0 on success, non-zero if invalid" — agrees with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- The packed encoding and the field ceilings are invariant across TOS versions
  and EmuTOS builds.
- The RTC target of the persist step varies by `CONF_WITH_*RTC*`/
  `CONF_WITH_IKBD_CLOCK`; the Atari standard path is the IKBD clock.

---

## NewTOS implementation notes

(Empty during Phase 00.)

NewTOS drives persistence through its own RTC abstraction (PL031 on QEMU; a
software clock on RPi4). Keep the field ceilings and the 2-second quantisation
so ported programs that round-trip `Tsettime`/`Tgettime` behave identically.

---

## Pi / BCM2711 binding

> Re-points the commit target from the **RTC (via XBIOS `Settime`) + the
> `current_time` cache** to the **seeded software epoch**. The validation
> contract (`sec/2 >= 30` or `min >= 60` → `ERROR`, hour unvalidated per EmuTOS;
> `E_OK` otherwise) and the 2-second quantisation are unchanged. See
> `docs/HARDWARE-PROFILE.md` §2 (RTC row), §3.5.

### `Tsettime` recomputes `BASE_SECS`

`Tsettime` (0x2D) validates the packed time (`time.rs:174-182`; `dsec >= 30`
or `min >= 60` → `ERROR`, hour not validated — EmuTOS parity), then recomputes
the base epoch keeping the current date: `BASE_SECS = compose(cur.date, hour,
min, dsec*2) - CNTPCT/CNTFRQ` (`time.rs:184-187`). The `sec/2` field is stored
as whole seconds (`dsec*2`), preserving the 2-second resolution. No RTC write;
nothing persists.

| Atari reference | Pi-4 model today |
|-----------------|------------------|
| Validated, committed through XBIOS `Settime` → `current_time` + RTC | validated, then `BASE_SECS` recomputed (single shared clock) |
| Persists in the RTC | **not persisted** — resets each power-up |

### Gaps vs. the contract (Step-2 candidates)

- **No persistence** — shared item with `Settime`/`Gettime` (needs an RTC
  backend). Validation, the 2-second quantisation, and `E_OK`/`ERROR` are fully
  implemented.
