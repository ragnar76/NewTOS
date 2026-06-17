# VDI text-metric inquiries (Phase 00b D5)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_text.c` (`vdi_vqt_extent`,
> `vdi_vqt_width`, `vdi_vqt_name`, `vdi_vqt_fontinfo`). Calling
> convention: [`../_dispatcher.md`](../_dispatcher.md). All read the
> current [`Vwk`](../../structures/vwk.md) font/effects state; no error
> return (except `vqt_width`'s out-of-range marker). `flip_y = 1` where
> noted.

| Op | Fn | Input | Output |
|----|----|-------|--------|
| 116 | `vqt_extent`   | `CONTRL[3]`=count, `INTIN[]`=char codes | bounding box of the string, 4 corner points |
| 117 | `vqt_width`    | `INTIN[0]`=char code | char cell + horizontal offsets |
| 130 | `vqt_name`     | `INTIN[0]`=font index (1-based) | font id + name |
| 131 | `vqt_fontinfo` | — | current font metrics |

## `vqt_extent` (116)

Computes `width = calc_width(count, INTIN)`, `height = calc_height()`,
then writes the **four corner points** of the rotated bounding box into
`PTSOUT[0..7]` according to `chup` (0 / 900 / 1800 / 2700). For
unrotated text: P1=(0,0)…P3=(width,0)…P5=(width,height)… i.e. the box
walks the rectangle in the rotated frame. `flip_y = 1`.

## `vqt_width` (117)

For char `INTIN[0]`:
- Out of `[first_ade, last_ade]` → `INTOUT[0] = -1` (no such char).
- Else `INTOUT[0] = char`, `PTSOUT[0]` = cell width from the font
  `off_table` (`off_table[k+1]-off_table[k]`), scaled by `dda_inc` /
  `act_siz` if `scaled`. `PTSOUT[2]`/`PTSOUT[4]` = left/right horizontal
  offsets (`hor_table`, only if `F_HORZ_OFF`), else 0. `flip_y = 1`.

## `vqt_name` (130)

Walks the `font_ring` chain counting **distinct font ids**; the
`INTIN[0]`-th (1-based) distinct face is selected. Out of range →
system font (`fon6x6`). Returns `INTOUT[0]` = font id, then the face
name as one char per WORD in `INTOUT[1..]`, zero-padded to
`FONT_NAME_LEN`. (Does **not** change `cur_font`.)

## `vqt_fontinfo` (131)

Current-font metrics (`INTOUT[0..1]` = `first_ade`/`last_ade`;
`PTSOUT[]` = max cell width, bottom, effects-adjusted thicken/offsets,
descent, half, ascent, top). Skew/thicken slots reflect the active
`style` bits. `flip_y = 1`.

## NewTOS implementation notes

- With the single bring-up bitmap font, `vqt_name` reports one face and
  `vqt_extent`/`vqt_width` use that font's fixed cell metrics (×2 when
  integer-doubled). The `font_ring`/`off_table`/`hor_table` machinery
  stays in the spec for when a real font subsystem lands.
- These are read-only metric queries; no framebuffer access.
- Rotation handling in `vqt_extent` mirrors
  [`vst_rotation`](../attributes/_text.md) — same 90° snapping.
