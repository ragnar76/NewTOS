# `Settime` (XBIOS 0x16)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/clock.c:1218` (`settime`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, `void` return, the DOS-packed datetime encoding, the
> "no validation" behaviour, that it updates the GEMDOS `current_time`/
> `current_date` globals. Reference: the RTC backend chain.

## EmuTOS source

**Primary location:** `bios/clock.c:1218` — `void settime(LONG time)`, slot
0x16 of `xbios_vecs[]` (`bios/xbios.c:1312`). This is the XBIOS function the
GEMDOS date/time setters commit through (`#define Settime(a) xbios_v_l(22,a)`,
`include/xbiosbind.h:40`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called (RTC backends, one per build):**
- `bios/clock.c` — `nsetdt` (NVRAM), `msetdt` (MegaRTC), `monstersetdt`,
  `icdsetdt`, `ultrasatan_setdt`, `isetdt` (IKBD clock) — write the RTC
  hardware (`bios/clock.c:1224-1263`).

**Headers / type definitions used:**
- `include/portab.h` — `LOWORD`/`HIWORD`.

**System variables / globals read:** none.

**System variables / globals written:**
- `current_time = LOWORD(time)` (`bios/clock.c:1221`).
- `current_date = HIWORD(time)` (`bios/clock.c:1222`). These are the GEMDOS
  cache globals (`bdos/time.c:53`) that `Tgetdate`/`Tgettime` read.
- the RTC hardware (per backend).

**Build configuration affecting this function:**
- `CONF_WITH_NVRAM`, `CONF_WITH_MEGARTC`, `CONF_WITH_MONSTER`,
  `CONF_WITH_ICDRTC`, `CONF_WITH_ULTRASATAN_CLOCK`, `CONF_WITH_IKBD_CLOCK` —
  which RTC (if any) is written. With **none** compiled in, `Settime` only
  updates the GEMDOS cache and does not persist.

---

## Source extraction summary

`Settime` sets the system clock from a 32-bit DOS-format value (time in the
low word, date in the high word). It **always** writes the GEMDOS-visible
cache first — `current_time = LOWORD(time)`, `current_date = HIWORD(time)` —
then dispatches to whichever RTC the machine has (NVRAM → MegaRTC → MonSTer →
ICD → UltraSatan → IKBD, in that detection order), returning after the first
match. On a build with no RTC backend it updates only the cache.

It does **no** range validation: a program can pass an impossible date and it
is accepted into the cache and (if present) the RTC. GEMDOS `Tsetdate`/
`Tsettime` (0x2B/0x2D) validate and then commit through `Settime`, so they are
the safer entry point. There is no return value and no error report — RTC
write failures are not surfaced.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x16
- **Stack frame on entry (m68k):** `Settime(time)` = `xbios_v_l(22, a)`
  (`include/xbiosbind.h:40`):
  ```
  +2: time (LONG)
   0: function_id (word) = 0x0016
  ```
- **Total parameter bytes:** 6 (L).
- **Native (AArch64) ABI:** ID `0x16` in `x8`; arg in `x0`; **`void`** — no
  return value.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `time` | `LONG` (32) — packed datetime |

---

## Inputs

| Position | Name   | Type   | Size | Source location    | Notes |
|----------|--------|--------|------|--------------------|-------|
| arg0     | `time` | `LONG` | 4    | `bios/clock.c:1221`| high word = date, low word = time |

### Packed datetime (same as `Tgetdate`/`Tgettime`)

- `HIWORD(time)` (date): bits 0-4 day (1-31), 5-8 month (1-12), 9-15
  year-since-1980 (0-127).
- `LOWORD(time)` (time): bits 0-4 double-seconds (`sec/2`, 0-29), 5-10 minute
  (0-59), 11-15 hour (0-23).

---

## Outputs

### Return value

- **Register:** none (`void`).
- **Type:** `void`.
- **Source location:** `bios/clock.c:1218` (declared `void`).

### Side effects on global state

- `current_time`/`current_date` updated (`bios/clock.c:1221-1222`).

### Side effects on hardware

- Writes the RTC chip (per backend), or none on a no-RTC build.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

No validation; RTC write failures are not reported.

---

## Verification criteria

- **Cache updated:** after `Settime((date<<16)|time)`, a subsequent
  `Gettime()`/`Tgetdate()`/`Tgettime()` reflects the new packed value.
- **No validation:** `Settime` with an out-of-range field (e.g. month 14) is
  accepted into the cache (unlike `Tsetdate`, which rejects it — see D-007).
- **No return:** the call yields no error code even if there is no RTC.
- **Persistence (RTC build):** on a build with an RTC, the value survives into
  the RTC and is re-read at next boot.

---

## Edge cases discovered in source

1. **Cache written before the RTC, unconditionally.** `current_time`/
   `current_date` are set first (`bios/clock.c:1221-1222`), so GEMDOS time is
   updated even if the RTC write later fails or there is no RTC.
2. **No validation.** Any packed value is accepted (`Settime` is the raw
   setter); GEMDOS `Tsetdate`/`Tsettime` add the bounds checks (D-007).
3. **No-RTC build only updates the cache.** With all RTC `CONF_WITH_*` off,
   nothing persists; on the NewTOS profile (no IKBD clock either) time resets
   to the boot default each power-up unless NewTOS supplies its own RTC.
4. **Falcon RTC year offset quirks** are handled inside the backends, not
   here — reference only.

---

## Dependencies

### Requires (must be initialized / functional)

- `current_time`/`current_date` (shared with GEMDOS §12).
- One RTC backend for persistence (optional).

### Used by

- GEMDOS `Tsetdate`/`Tsettime`, AES control panel, time-sync utilities.

---

## Related data structures

- `docs/api/gemdos/_state.md §12` — `current_time`/`current_date` + packing.
- `docs/api/xbios/17-gettime.md` — the reader.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the packed
  datetime; the "no validation" is EmuTOS behaviour.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- One RTC backend must be enabled for persistence. NewTOS supplies its own
  RTC path (below); the packed encoding is unchanged.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: `Settime` updates the kernel clock cache and writes the AArch64 RTC
— PL031 on QEMU `aarch64-virt`; on RPi4 (no on-board RTC) NewTOS persists to
a tagged SD/config region or an I²C RTC if present. The GEMDOS `current_time`/
`current_date` update is mandatory; the packed encoding is contract.
