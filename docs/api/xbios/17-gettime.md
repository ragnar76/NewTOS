# `Gettime` — XBIOS 0x17

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/clock.c:1266` (`gettime`).

## EmuTOS source (essence)

```c
LONG gettime(void)
{
#ifdef MACHINE_AMIGA   return amiga_getdt();
#endif
#ifdef MACHINE_LISA    return lisa_getdt();
#endif
#if CONF_WITH_NVRAM    if (has_nvram)            return ngetdt();
#endif
#if CONF_WITH_MEGARTC  if (has_megartc)          return mgetdt();
#endif
#if CONF_WITH_MONSTER  if (has_monster_rtc)      return monstergetdt();
#endif
#if CONF_WITH_ICDRTC   if (has_icdrtc)           return icdgetdt();
#endif
#if CONF_WITH_ULTRASATAN_CLOCK
                       if (has_ultrasatan_clock) return ultrasatan_getdt();
#endif
#if CONF_WITH_IKBD_CLOCK
                       return igetdt();
#endif
    return MAKE_ULONG(current_date, current_time);  /* last-resort fallback */
}
```

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x17` |
| WORD args | 0 |
| Stack frame | `+0: 0x0017` |
| Vector | `xbios_vecs[23]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (packed datetime) |

High 16 bits = date (Tgetdate encoding); low 16 bits = time
(Tgettime encoding).

## Errors

None.

## Edge cases

### 1. Reads RTC, not cache

Unlike GEMDOS `Tgetdate`/`Tgettime` (which read the cached
`current_date`/`current_time`), XBIOS `Gettime` reads the **RTC
hardware** directly. This is slower (I²C / IKBD command + reply)
but always current.

### 2. Cache vs RTC drift

A program calling `Tgetdate` after `Settime` sees the new value
immediately (cache updated first). A program calling `Gettime`
reads the RTC, which may be a few microseconds behind because the
RTC chip's internal counter has a granularity of 1 second.

### 3. IKBD command latency

The IKBD-clock path sends `0x1C` (read clock) over the 7812 baud
ACIA, awaits a 7-byte reply via interrupt. Latency: ~2 ms. EmuTOS
blocks (in supervisor mode) the calling task. NewTOS without IKBD
hardware skips this.

### 4. NewTOS fallback path

With all `CONF_WITH_*_CLOCK` disabled and no IKBD chip,
`gettime()` reaches the **last-resort** `MAKE_ULONG(current_date,
current_time)` — i.e. it returns the GEMDOS cache. Since `Settime`
also updates the cache, the round-trip Set→Get works, but time
does **not advance** between sets unless the BIOS tick handler
increments `current_time`/`current_date`. PHASE-07 must ensure
the AArch64 timer IRQ updates these.

### 5. RTC time format

The Atari clock encoding survives the round-trip. Year offset is
1980-based.

## Dependencies

- One of the `*getdt()` backends (per RTC).
- `current_time`, `current_date` (fallback / NewTOS path).

Used by: GEMDOS time getters via cache, programs needing wall-
clock time directly from RTC, AES message-time tagging.

## Cross-references

- **Compendium:** "Gettime" — agrees.
- **Profibuch:** "Gettime (0x17)" — agrees.
- `docs/api/xbios/16-settime.md`
- `docs/api/gemdos/time/2a-tgetdate.md`

## Configuration

- See Settime notes — exactly one RTC backend needs to be active
  for hardware time. Otherwise falls through to GEMDOS cache.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: implement `gettime` to read the AArch64 generic timer
or PL031 on QEMU. On RPi4 without RTC, return the GEMDOS-cached
value updated from boot-time-set + monotonic-since-boot delta.
