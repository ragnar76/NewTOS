# `Physbase` — XBIOS 0x02

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:1042` (`physbase`).

## EmuTOS source

```c
const UBYTE *physbase(void)
{
#ifdef MACHINE_AMIGA   return amiga_physbase();
#elif defined(MACHINE_LISA) return lisa_physbase();
#elif CONF_WITH_ATARI_VIDEO return atari_physbase();
#else                       return logbase();   /* fallback */
#endif
}
```

`atari_physbase()` (file-local in `screen.c`) reads the shifter's
video-base registers (`0xFF8201` / `0xFF8203` / `0xFF820D` for
ST/STe) and reconstructs the 24-bit address. On STe and later it
also folds in the lower-byte register `0xFF820D`.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x02` |
| WORD args | 0 |
| Stack frame | `+0: 0x0002` |
| Vector | `xbios_vecs[2]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (`UBYTE *`) |

Pointer to the start of the **physical** screen — what the video
hardware is currently displaying. May differ from `Logbase()` if
double-buffering is in use.

## Errors

None — function cannot fail. Returns whatever the hardware
register reads back (no validation).

## Edge cases

- **STe alignment**: Atari ST hardware demands 256-byte alignment
  for `phys` (Shifter base register granularity). STe extends to
  byte alignment via the lower-byte register. NewTOS must preserve
  the exact value passed to `Setscreen` regardless of alignment.

- **Double-buffer convention**: `Logbase()` returns where future
  drawing will land; `Physbase()` returns what's currently on
  screen. After `Setscreen(log, phys, …)` they may differ until
  the next `Vsync` — programs use this to flip pages.

- **No video hardware** (NewTOS HDMI-only profile): EmuTOS falls
  back to `Logbase()`. This means physbase == logbase, no real
  page-flipping at the hardware level. NewTOS will implement
  proper page-flipping via the VideoCore framebuffer API.

## Dependencies

- `atari_physbase` ⇒ shifter hardware registers.
- (NewTOS) framebuffer driver, mailbox channel.

Used by: graphics double-buffering, screen savers, hardware-cursor
emulation.

## Cross-references

- **Compendium:** "Physbase" — agrees.
- **Profibuch:** "Physbase (0x02)" — agrees.
- `docs/api/xbios/03-logbase.md`
- `docs/api/xbios/05-setscreen.md`

## Configuration

- `CONF_WITH_ATARI_VIDEO` — without it, fallback to `logbase()`.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: physbase = current "displayed" framebuffer in the
VideoCore swap chain. After `Setscreen(log,phys,…)` the next
mailbox-driven flip aligns the two on the next vblank.
