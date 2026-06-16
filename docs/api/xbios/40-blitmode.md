# `Blitmode` — XBIOS 0x40

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/xbios.c:886` (`blitmode`).

## EmuTOS source

```c
static WORD blitmode(WORD mode)
{
#if CONF_WITH_BLITTER
    WORD status = 0x0000;
    if (has_blitter) {
        status = blitter_is_enabled ? 0x0003 : 0x0002;
        if (mode != -1)  blitter_is_enabled = mode & 1;
    }
    return status;
#else
    return 0x0000;
#endif
}
```

`has_blitter` is detected at boot by probing the blitter MMIO
register. `blitter_is_enabled` is a software flag consulted by the
line-A and VDI bit-blit routines.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x40` |
| WORD args | 1 |
| Stack frame | `+2: mode (W)`, `+0: 0x0040` |
| Vector | `xbios_vecs[64]` |

## Inputs

| Pos | Name   | Type | Range / meaning                                  |
|-----|--------|------|--------------------------------------------------|
| 0   | `mode` | WORD | `-1` query / `0` software / `1` hardware blitter  |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended WORD) |

Bit-encoded status of the **previous** state:

| Bit | Meaning                          |
|----:|----------------------------------|
| 0   | hardware blitter currently ON     |
| 1   | hardware blitter is **present**   |

| Hardware | `blitter_is_enabled` | Return |
|----------|----------------------|-------:|
| absent   | (n/a)                | `0x0000` |
| present  | 0 (software)         | `0x0002` |
| present  | 1 (hardware)         | `0x0003` |

## Errors

None.

## Edge cases

### 1. NewTOS profile

`CONF_WITH_BLITTER` disabled — return is always `0x0000`.
Programs detecting "no blitter" via this XBIOS work correctly.
Programs **assuming** the blitter exists will fall through to
software bit-blit (line-A routines).

### 2. `mode = -1` is query

Standard query idiom. Returns status without changing
`blitter_is_enabled`.

### 3. Mask `mode & 1`

Only the low bit of `mode` is consumed for the new state. Passing
`mode = 2` therefore *disables* the hardware blitter (low bit is
0). Programs should pass `0` or `1` only.

### 4. Old VDI / line-A path

When the blitter is enabled, line-A bit-blit calls (e.g. `bitblt`)
program the blitter MMIO and start it; otherwise they execute the
m68k software equivalent. The runtime cost difference is large
(20× faster with blitter on STe). NewTOS line-A always runs
software (no blitter).

### 5. Falcon Blitmode extension

Falcon TOS adds bits 2..3 (Falcon-specific blitter modes). EmuTOS
does not implement; bits remain 0. Cross-reference, not
discrepancy.

## Dependencies

- `has_blitter`, `blitter_is_enabled` (`bios/blitter.c`).

Used by: GEM (line-A bit-blits), demos that explicitly want
software blits for special effects.

## Cross-references

- **Compendium:** "Blitmode" — agrees.
- **Profibuch:** "Blitmode (0x40)" — agrees.

## Configuration

- `CONF_WITH_BLITTER`. NewTOS: not present.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: NewTOS may eventually add a hardware accelerator (V3D /
DMA-blitter on the BCM2711) and re-expose it through `Blitmode`.
For now: hard `0x0000`. Programs that ignore `Blitmode` and just
call line-A `bitblt` get software speed — about 50× faster than ST
hardware on the Pi 4 anyway.
