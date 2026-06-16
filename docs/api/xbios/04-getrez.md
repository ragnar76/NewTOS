# `Getrez` — XBIOS 0x04

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/screen.c:1076` (`getrez`).

## EmuTOS source

```c
WORD getrez(void)
{
#if CONF_WITH_ATARI_VIDEO
    return atari_getrez();
#else
    return sshiftmod;          /* logical mode cache */
#endif
}
```

`atari_getrez()` reads the shifter resolution register
(`0xFF8260` for ST/STe; the TT and Falcon registers when present).
`sshiftmod` is a software shadow used when there is no hardware to
query.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x04` |
| WORD args | 0 |
| Stack frame | `+0: 0x0004` |
| Vector | `xbios_vecs[4]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (zero-extended WORD) |

### Atari resolution constants (`include/biosdefs.h:124-131`)

| Value | Constant     | Meaning                              |
|------:|--------------|--------------------------------------|
| 0     | `ST_LOW`     | 320 × 200, 16 colours (ST/STe)       |
| 1     | `ST_MEDIUM`  | 640 × 200, 4 colours                  |
| 2     | `ST_HIGH`    | 640 × 400, 2 colours (mono)          |
| 3     | `FALCON_REZ` | (Falcon indicator — see Vsetmode)    |
| 4     | `TT_MEDIUM`  | 640 × 480, 16 colours (TT)           |
| 6     | `TT_HIGH`    | 1280 × 960, mono                     |
| 7     | `TT_LOW`     | 320 × 480, 256 colours               |

Values 5 and 8..255 are reserved.

## Errors

None.

## Edge cases

- **Falcon ambiguity**: on a Falcon, `Getrez` returns `FALCON_REZ`
  (3) for any Falcon-specific mode; `Vsetmode(-1)` gives the actual
  Videl mode word. NewTOS does not implement Videl, so this is
  cross-reference only.

- **Mono-only on TT_HIGH**: returning 6 implies a mono monitor is
  attached — applications switch palettes accordingly.

- **No "no monitor" state**: there is no defined return value for
  "display unconfigured". Whatever rez was last set, that's what
  Getrez reports.

- **NewTOS startup**: until PHASE-08 brings up the video pipeline,
  Getrez can return either `sshiftmod` (a build-time default) or
  the closest-match value derived from the actual framebuffer
  geometry chosen by VideoCore.

## Dependencies

- `atari_getrez` or `sshiftmod`.

Used by: AES (palette init), VDI workstation open, screen-aware
applications.

## Cross-references

- **Compendium:** "Getrez" — agrees.
- **Profibuch:** "Getrez (0x04)" — agrees, lists Falcon caveat.
- `docs/api/xbios/05-setscreen.md`
- `docs/api/xbios/_state.md` (pending) — `sshiftmod`, `v_bas_ad`.

## Configuration

- `CONF_WITH_ATARI_VIDEO`, `CONF_WITH_TT_SHIFTER`,
  `CONF_WITH_VIDEL` — affect which constants are reachable.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08 plan: NewTOS reports a synthesised TT mode based on the
configured framebuffer:
- 320×200 → `ST_LOW`
- 640×400 → `ST_HIGH`
- 640×480 256-col → `TT_LOW`
- 1280×960 mono → `TT_HIGH`

Native HD/4K modes will report `TT_MEDIUM` until a NewTOS-specific
extension call exposes the real geometry.
