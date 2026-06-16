# `Settime` — XBIOS 0x16

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/clock.c:1218` (`settime`).

## EmuTOS source

```c
void settime(LONG time)
{
    current_time = LOWORD(time);    /* update GEMDOS cache */
    current_date = HIWORD(time);

#if CONF_WITH_NVRAM
    if (has_nvram)            { nsetdt(time);          return; }
#endif
#if CONF_WITH_MEGARTC
    if (has_megartc)          { msetdt(time);          return; }
#endif
#if CONF_WITH_MONSTER
    if (has_monster_rtc)      { monstersetdt(time);    return; }
#endif
#if CONF_WITH_ICDRTC
    if (has_icdrtc)           { icdsetdt(time);        return; }
#endif
#if CONF_WITH_ULTRASATAN_CLOCK
    if (has_ultrasatan_clock) { ultrasatan_setdt(time); return; }
#endif
#if CONF_WITH_IKBD_CLOCK
    isetdt(time);
#endif
}
```

The function is the **single point of truth** for setting the
system clock. It updates the GEMDOS-visible cached `current_time`
/ `current_date` first, then dispatches to whichever RTC the
hardware has — Mega ST RTC, NVRAM (Falcon), MonSTer board, ICD
clock, UltraSatan clock, or IKBD clock as fallback.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x16` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: time (LONG)`, `+0: 0x0016` |
| Vector | `xbios_vecs[22]` |

## Inputs

| Pos | Name   | Type | Range / meaning                                       |
|-----|--------|------|-------------------------------------------------------|
| 0   | `time` | LONG | packed datetime: high 16 bits = date, low 16 = time   |

`HIWORD(time)`: bits 15..9 year-since-1980, 8..5 month, 4..0 day.
`LOWORD(time)`: bits 15..11 hour, 10..5 minute, 4..0 dbl-second.
(Same encoding as `Tgetdate`/`Tgettime`.)

## Outputs

`void` — no return. D0 == 0 from dispatcher default.

## Errors

None — settime is unconditional. The RTC drivers do not report
write failures back through this interface.

## Edge cases

### 1. No validation

`Settime` does **no** range checking. A program can pass
`(month=14, day=31)` and the cached `current_date` accepts it; the
RTC chip stores it; `Tgetdate` returns it. Sanity-checking is the
caller's responsibility — `Tsetdate` (GEMDOS 0x2B) does validate
and is the safer entry point.

### 2. Atomicity GEMDOS vs RTC

Update order: cached values first, then RTC. A `Tgettime` racing
with `Settime` may see new cached time but old RTC state — but
this only matters if something later reads from the RTC directly
(not the case in EmuTOS GEMDOS path).

### 3. RTC priority

The fallback chain `NVRAM → MEGARTC → MONSTER → ICDRTC →
ULTRASATAN → IKBD` is the EmuTOS detection order. NewTOS profile
disables `CONF_WITH_NVRAM`/`MEGARTC`/`MONSTER`/`ICDRTC`/
`ULTRASATAN` and disables `IKBD_CLOCK` too (no IKBD chip on
RPi4) — so on stock NewTOS profile, `Settime` only updates
`current_time`/`current_date` and **does not persist**.

### 4. Persistence vs runtime

EmuTOS bootstraps `current_time` from the RTC at boot. NewTOS will
need a real RTC source (see notes); without one, time resets to
build-default on every boot.

### 5. `Tsetdate`/`Tsettime` route through here

GEMDOS time setters call `Settime` with a re-packed full datetime,
preserving the field they aren't changing. A naive call to
`Settime` from a user program bypasses `Tsetdate`'s validation —
intentional escape hatch for AES.

## Dependencies

- `current_time`, `current_date` globals (shared with GEMDOS).
- One of: `nsetdt`, `msetdt`, `monstersetdt`, `icdsetdt`,
  `ultrasatan_setdt`, `isetdt` (per build config).

Used by: GEMDOS `Tsetdate`/`Tsettime`, AES control panel, NTP-
sync utilities.

## Cross-references

- **Compendium:** "Settime" — agrees.
- **Profibuch:** "Settime (0x16)" — agrees.
- `docs/api/xbios/17-gettime.md`
- `docs/api/gemdos/time/2b-tsetdate.md`,
  `docs/api/gemdos/time/2d-tsettime.md`

## Configuration

- One RTC backend must be enabled for persistence. NewTOS will
  need to add a new RTC backend (see notes) since none of the
  EmuTOS options apply.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07 plan: `Settime` updates the cached values and writes to
the AArch64 RTC — on QEMU `aarch64-virt`, the PL031; on RPi4,
the only true RTC is via I²C-attached PCF85063 on Pi 5 (RPi4 has
none — boot timestamp comes from network/file). NewTOS will keep
"persisted time" in a tagged region of the SD-card boot config or
in CMOS-equivalent NVMEM if available, with PL031 as primary on
QEMU.
