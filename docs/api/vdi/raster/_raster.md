# VDI raster operations (Phase 00b D6)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_raster.c` (`vdi_vro_cpyfm`,
> `vdi_vrt_cpyfm`, `vdi_vr_trnfm`, `cpy_raster`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). These move bitmap blocks
> between **MFDB**s (memory form definition blocks). No error return;
> invalid params silently no-op.

| Op | Fn | Input | Effect |
|----|----|-------|--------|
| 109 | `vro_cpyfm` | MFDBs + rects + `INTIN[0]` logic op | copy raster **opaque** (colour→colour, all planes) |
| 121 | `vrt_cpyfm` | MFDBs + rects + `INTIN[0..2]` | copy raster **transparent** (1-plane mono → colour) |
| 110 | `vr_trnfm`  | src + dst MFDB | transform between **standard** and **device** form |

## MFDB — memory form definition block

The source and destination MFDB **pointers** are passed in the control
array: `CONTRL[7..8]` = source MFDB, `CONTRL[9..10]` = destination MFDB
(each a 32-bit pointer split across two WORDs). Layout
(`vdi_raster.c:166`):

| Field | Meaning |
|-------|---------|
| `fd_addr` | pixel data address (NULL ⇒ the screen) |
| `fd_w` / `fd_h` | width / height in pixels |
| `fd_wdwidth` | width in **words** (`(fd_w+15)/16`) |
| `fd_stand` | 0 = device-specific, 1 = standard (plane-interleaved) form |
| `fd_nplanes` | bit-plane count |
| `fd_r1/r2/r3` | reserved |

## `vro_cpyfm` (109) — opaque copy

1. `arb_corner` both rectangles: `PTSIN[0..3]` = source, `PTSIN[4..7]`
   = destination.
2. `mode = INTIN[0]` — a **16-entry BitBlt logic op** (0..15, e.g.
   3 = S→D replace, 6 = XOR, 7 = OR). Bit 5 (`PAT_FLAG`) selects
   pattern-blit. `mode & ~0x1f` ⇒ invalid, no-op.
3. Source and destination must have **equal plane counts**.
4. Blits all planes via `bit_blt` (or `fast_bit_blt`), clipped to the
   `Vwk` clip rect.

## `vrt_cpyfm` (121) — transparent copy

Copies a **1-plane (monochrome)** source into a colour destination,
expanding 1-bits to a foreground colour and 0-bits to a background
colour (or leaving them, per mode):

- `INTIN[0]` = write mode: `MD_TRANS` (1), `MD_REPLACE` (2),
  `MD_XOR` (3), `MD_ERASE` (4); other ⇒ no-op.
- `INTIN[1]` = foreground colour idx, `INTIN[2]` = background colour
  idx (both `validate_color_index` → `MAP_COL`).
- Source MFDB **must** be one plane (`s_nxwd == 2` bytes/word-row).

Each mode sets a 4-entry `op_tab` (the per-(fg,bg)-bit logic op) — e.g.
`MD_TRANS` ORs fg pixels in, `MD_XOR` XORs, `MD_REPLACE` writes fg/bg
for 1/0 bits.

## `vr_trnfm` (110) — form transform

Converts the source bitmap between **standard** (plane-sequential,
machine-independent) and **device-dependent** (interleaved) layout.
`fd_stand` of the destination is forced to the opposite of the source.
Works in place if src and dst addresses coincide. Plane size =
`fd_h * fd_wdwidth` words.

## NewTOS implementation notes

- `bit_blt` maps to the framebuffer block-copy primitive. The 16 logic
  ops are the classic GEM/BitBlt raster ops — implement the full table
  (NOT just replace) since programs rely on XOR (rubber-banding) etc.
  Target/source framebuffer or MFDB memory; screen = `vwk->device`
  when `fd_addr == NULL` (ADR-011).
- `vr_trnfm` matters for loading icon/image data authored in standard
  form; keep both directions.
- The MFDB struct is a **fixed ABI** parsed by ported programs — match
  the field order/size exactly (unlike the internal `Vwk`).
- Truecolor paths (`CONF_WITH_VDI_16BIT`) are out of bring-up scope
  (8-bit palette screen first).
