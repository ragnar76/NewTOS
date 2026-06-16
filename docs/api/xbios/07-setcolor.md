# `Setcolor` — XBIOS 0x07

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:1192` (`setcolor`).

## EmuTOS source

```c
WORD setcolor(WORD colorNum, WORD color)
{
#ifdef MACHINE_AMIGA   return amiga_setcolor(colorNum, color);
#elif CONF_WITH_ATARI_VIDEO   return atari_setcolor(colorNum, color);
#else                  return 0;          /* fake */
#endif
}
```

`atari_setcolor()`:
1. Reads existing palette[colorNum] (with mask for ST 9-bit vs STe
   12-bit).
2. If `color >= 0`, writes it.
3. Returns the previous value.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x07` |
| WORD args | 2 (WORD + WORD) |
| Stack frame | `+4: color (W)`, `+2: colorNum (W)`, `+0: 0x0007` |
| Vector | `xbios_vecs[7]` |

## Inputs

| Pos | Name        | Type | Range / meaning                              |
|-----|-------------|------|----------------------------------------------|
| 0   | `colorNum`  | WORD | palette index 0..15                          |
| 1   | `color`     | WORD | new colour value, or `-1` for query-only      |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended WORD) |

The previous palette value at `colorNum`, masked to 9 bits (ST) or
12 bits (STe).

## Errors

None directly. `colorNum` out of range (≥ 16) writes outside the
palette array — undefined behaviour.

## Edge cases

### 1. `color == -1` is query

Standard query idiom. Returns previous palette[colorNum] without
modifying anything.

### 2. Mask depends on hardware

ST: `0x0777` mask (3 bits per channel).
STe: `0x0FFF` mask, but with STe bit-reordering `xRRRrGGGgBBBb`.
EmuTOS picks the mask via `has_ste_palette` at runtime.

### 3. No range check on `colorNum`

`colorNum > 15` corrupts memory adjacent to the palette array. ST
hardware silently clamps to bits 0..3, but the software palette
shadow `palette[]` has only 16 entries.

### 4. Synchronisation with VBL

Like Setpalette, the change becomes visible at the next VBL.
Calling repeatedly within a frame does not cause flicker on
hardware (the shifter only latches on horizontal sync) but on
NewTOS framebuffer emulation a flush-to-CLUT must be coalesced.

### 5. Falcon mode

In Falcon true-color mode, Setcolor accepts a packed 16-bit RGB
value (`5R6G5B`). EmuTOS's `atari_setcolor` clamps to ST/STe
behaviour; Videl path uses Esetcolor (XBIOS 0x53). NewTOS without
Videl: only ST/STe encoding.

## Dependencies

- `atari_setcolor` ⇒ palette[] + shifter MMIO.

Used by: Setpalette callers' single-entry updates, AES desk-
accessory colour negotiation.

## Cross-references

- **Compendium:** "Setcolor" — agrees on `-1` query.
- **Profibuch:** "Setcolor (0x07)" — agrees.
- `docs/api/xbios/06-setpalette.md`

## Configuration

- `CONF_WITH_ATARI_VIDEO`.

## NewTOS implementation notes

(Empty during Phase 00.)
