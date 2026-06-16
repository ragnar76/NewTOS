# `Setpalette` — XBIOS 0x06

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:1168` (`setpalette`).

## EmuTOS source

```c
void setpalette(const UWORD *palettePtr)
{
#ifdef MACHINE_AMIGA   amiga_setpalette(palettePtr);
#elif CONF_WITH_ATARI_VIDEO  atari_setpalette(palettePtr);
#endif
}
```

`atari_setpalette()` copies 16 UWORDs to the shifter palette
registers (`0xFF8240..0xFF825F`) on the next VBL. STe encoding is
4-bit-per-channel (`0x0RGB`) packed as `xRRRrGGGgBBBb` (high bit
extension).

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x06` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: palettePtr (LONG)`, `+0: 0x0006` |
| Vector | `xbios_vecs[6]` |

## Inputs

| Pos | Name          | Type           | Range / meaning                          |
|-----|---------------|----------------|------------------------------------------|
| 0   | `palettePtr`  | const UWORD *  | pointer to 16-entry palette array         |

The array contains 16 UWORDs regardless of current resolution —
high resolutions use only the first 2 or 4 entries, but the call
always copies all 16.

## Outputs

`void` — no return. D0 will be 0 (or whatever the dispatcher
defaulted to).

## Errors

None. NULL pointer dereferences.

## Edge cases

### 1. Synchronisation with VBL

The hardware copies the new palette on the next VBL. Calling
Setpalette mid-frame causes a partial palette swap that affects
the bottom half of the screen. Programs wanting clean transitions
must `Vsync()` first.

### 2. Encoding

Atari ST: 3 bits per channel `0x0xRGB` (`0..7`).
Atari STe and later: 4 bits per channel via STe-specific bit
re-mapping (`xRRRrGGGgBBBb`). EmuTOS preserves the input bits
verbatim and lets hardware decode.

### 3. Pointer to ROM

Many programs use static palette tables in their TEXT segment
(ROM-mapped on cartridge software). Setpalette tolerates const-
pointer input by spec.

### 4. NULL pointer

EmuTOS does not check. Crashes / faults.

### 5. Concurrent Setcolor

Setpalette + Setcolor combined: Setpalette writes all 16, Setcolor
overwrites one. Order matters; last write wins.

## Dependencies

- `atari_setpalette` ⇒ shifter palette MMIO.

Used by: AES palette restoration on context switch, games, slide-
show programs (rapid palette swap for "more colours" effect).

## Cross-references

- **Compendium:** "Setpalette" — agrees.
- **Profibuch:** "Setpalette (0x06)" — agrees.
- `docs/api/xbios/07-setcolor.md`

## Configuration

- `CONF_WITH_ATARI_VIDEO` required.
- STe vs ST encoding via `has_ste_palette` runtime flag.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: emulate via writes to a VideoCore CLUT. STe encoding is
expanded to 8-bit-per-channel for the framebuffer hardware.
