# `Tickcal` — BIOS 0x06

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/bios.c:1097` (`tickcal`).

## EmuTOS source

```c
LONG tickcal(void) { return 20L; }   /* 50 Hz → 20 ms period */
```

The constant `20` reflects the 200 Hz / 50 Hz Atari heritage —
specifically the **system timer C** which fires at 200 Hz, but the
BDOS-visible "tick" is the 50 Hz frame interrupt. Per Compendium,
`Tickcal` returns "the period of the system timer in
milliseconds". For a 50 Hz timer, that's 20 ms.

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x06` |
| WORD args | 0 |
| Stack frame | `+0: 0x0006` |
| Vector entry | `bios_vecs[6]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG |

Returns `20` (decimal) — the timer period in milliseconds.

## Errors

None.

## Edge cases

### 1. Constant — never reflects actual hardware

EmuTOS hard-codes 20. If a system is built with `CONF_TIMER_C_HZ`
≠ 200 (none in EmuTOS upstream), the value would still be `20L`
and incorrect. NewTOS will likely keep returning `20` for ABI
purposes regardless of internal tick rate.

### 2. Programs **multiply** by Tickcal

The Atari `_hz_200` counter at address `0x4BA` increments at 200
Hz. To convert ticks → ms, programs do `ticks * 5` directly, or
`ticks * (Tickcal() / 4)` — but the canonical idiom is just
"Tickcal returns 20 so the 50-Hz tick is 20 ms". Programs that
read the `_hz_200` counter do not use Tickcal.

### 3. AES timer events

AES uses Tickcal to compute `evnt_timer` periods.

## Dependencies

None.

Used by: AES `evnt_timer`, programs computing real-time delays.

## Cross-references

- **Compendium:** "Tickcal" — agrees, value is 20.
- **Profibuch:** "Tickcal (0x06)" — agrees.
- `docs/api/bios/_state.md` — `_hz_200` system tick counter.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)
