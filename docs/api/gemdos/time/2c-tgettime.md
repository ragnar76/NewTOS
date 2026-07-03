# `Tgettime` (GEMDOS 0x2C)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/time.c:132` (`xgettime`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the packed-time WORD bit layout (2-second
> granularity), the zero-extend-to-LONG return shape. Reference: the
> `current_time` global and the tick handler that advances it.

## EmuTOS source

**Primary location:** `bdos/time.c:132` — `long xgettime(void)`. The body is a
single `return current_time;` — a pure read of the cached GEMDOS time word,
zero-extended to LONG. No I/O, no validation.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x2C` =
`xgettime`, `stdio_typ = 0`, `wparms = 0` (`bdos/bdosmain.c:234`).

**Helpers called:** none.

**Headers / type definitions used:**
- `bdos/time.c:53` — `current_time` (`UWORD`) declaration.
- `bdos/time.c:58-71` — time field masks + shifts (`SEC_BM`, `MIN_BM`,
  `HRS_BM`, `MIN_SHIFT`, `HRS_SHIFT`).

**System variables / globals read:**
- `current_time` (`bdos/time.c:53`) — the packed GEMDOS time WORD.

**System variables / globals written:** none.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Tgettime` returns the current system time as a FAT/DOS-packed WORD with
2-second resolution. EmuTOS keeps this in the file-local `current_time`
(`UWORD`), initialised at boot by `time_init` (`bdos/time.c:160`, from
`Gettime()`), advanced once per two seconds by the tick handler `tikfrk`
(`bdos/time.c:189`, `msec` accumulates 2000 ms then bumps the seconds field),
and overwritten by XBIOS `Settime` when the time is set. `xgettime` is a bare
read of the cache.

The C function is declared `long` and returns the `UWORD` directly, so the
result is **zero-extended**: the high 16 bits are always zero. There is no
error path.

The packed layout is the standard MS-DOS/FAT directory time: bits 0-4 hold
"double-seconds" (`seconds / 2`, 0-29), bits 5-10 minutes (0-59), bits 11-15
hours (0-23). Because seconds are stored `/2`, odd seconds are not
representable — `extract_time` (`bios/clock.c:88`) reconstructs seconds as
`(time << 1) & 0x3f`, i.e. always even.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x2C
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x002C
  ```
- **Total parameter bytes:** 2 (just the ID) → `wparms = 0`.
- **Native (AArch64) ABI:** ID `0x2C` in `x8`; no arguments; the packed-time
  WORD returned **zero-extended** into `x0` (bits 16-63 = 0).

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (a zero-extended packed-time UWORD)
- **Encoding:** MS-DOS/FAT packed time (see table); high 16 bits are zero.
- **Source location:** `bdos/time.c:134`.

### Packed time WORD encoding (2-second resolution)

| Bits   | Field         | Range / meaning                                  |
|-------:|---------------|--------------------------------------------------|
| 15-11  | hour          | 0-23                                             |
| 10-5   | minute        | 0-59                                             |
| 4-0    | "double-secs" | 0-29 → seconds 0-58 in steps of 2 (`sec = field × 2`) |

### Side effects on global state

None.

### Side effects on hardware

None — the value is read from a cached global, not from the RTC.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (none) | `bdos/time.c:132` | always the packed `current_time` |

`xgettime` never fails and never validates.

---

## Verification criteria

- **Nominal:** with `current_time` representing 14:30:52 (hour 14, minute 30,
  double-seconds `52/2 = 26`), the packed word is
  `(14 << 11) | (30 << 5) | 26 = 0x73BA`. `Tgettime()` returns `0x000073BA`
  in `x0` (high 16 bits zero), no side effects.
- **Midnight:** 00:00:00 packs to `0x0000`; `Tgettime()` returns `0`.
- **Odd second truncation:** setting 14:30:53 via `Tsettime` then reading back
  yields 14:30:52 (the LSB second is dropped) — the packed word cannot encode
  an odd second.
- **Round-trip:** `Tsettime(w)` (valid `w`) then `Tgettime()` returns the same
  `w`.

---

## Edge cases discovered in source

1. **2-second granularity.** The seconds field is `seconds / 2`; odd seconds
   cannot be represented. Programs needing finer resolution read the 200 Hz
   counter `_hz_200` and convert (`×5` ms), or use `Tickcal`
   (`docs/api/bios/06-tickcal.md`); `Tgettime` cannot resolve sub-2-second
   intervals.

2. **High word guaranteed zero.** The `UWORD`→`long` return zero-extends; the
   AArch64 result in `x0` has bits 16-63 = 0 (not sign-extended).

3. **Date/time can be read torn only under multitasking.** `tikfrk` updates
   `current_time` and (on midnight rollover) `current_date` in one uninterrupted
   pass (`bdos/time.c:189-229`). EmuTOS is single-threaded, so a
   `Tgettime` then `Tgetdate` pair is always coherent. NewTOS's preemptive
   scheduler must serialise the pair (or expose a combined snapshot) so a
   reader cannot observe a time that already rolled past midnight while the
   date has not — see NewTOS notes.

---

## Dependencies

### Requires (must be initialized / functional)

- `current_time` global, initialised by `time_init` (`bdos/time.c:160`) from
  `Gettime()` (XBIOS 23) at boot.
- The 200 Hz tick / `tikfrk` handler to advance the time.

### Used by

- `Fcreate`/`Fwrite` timestamping of FAT directory entries.
- `Fsfirst`/`Fsnext` DTA time field.
- `Fdatime` (0x57), shell `TIME`.

---

## Related data structures

- `docs/api/gemdos/_state.md` §12 — `current_date`/`current_time` globals + RTC
  path.
- `docs/api/gemdos/time/2d-tsettime.md` — the setter (same encoding).
- `docs/api/structures/dta.md` — the DTA `d_time` field uses this encoding.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically: "returns
  the current time in GEMDOS/DOS packed format" — agrees with EmuTOS.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- The packed encoding is invariant across TOS versions and EmuTOS builds.
- The boot-time value source varies by RTC hardware (see `2a-tgetdate.md`).

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: provide an atomic 32-bit fetch (`date << 16 | time`) for
multitasking-safe reading; `Tgettime`/`Tgetdate` then decompose the snapshot.
The tick source is the AArch64 generic timer.
