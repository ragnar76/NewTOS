# VDI line & marker attribute setters (Phase 00b D3)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_line.c`, `vdi/vdi_marker.c`.
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md). All
> setters follow the same shape: read input → validate (clamp to a
> range, else substitute a default) → store in the `Vwk` → **echo the
> accepted value** back in `INTOUT`/`PTSOUT`. None can fail (no VDI
> error return); an out-of-range request silently snaps to the default.

## Line attributes (`vdi_line.c`)

| Op | Fn | Input | Validation → `Vwk` | Output |
|----|----|-------|--------------------|--------|
| 15 | `vsl_type`  | `INTIN[0]` style | `1..MAX_LINE_STYLE` else `DEF_LINE_STYLE`; `line_index = v-1` | `INTOUT[0]` = accepted style |
| 16 | `vsl_width` | `PTSIN[0]` width | clamp `[1, SIZ_TAB[6]]`, then **force odd** (even → `w-1`); `line_width = w` | `PTSOUT[0]` = `w`, `PTSOUT[1]` = 0 |
| 17 | `vsl_color` | `INTIN[0]` colour | `validate_color_index`; `line_color = MAP_COL[i]` | `INTOUT[0]` = validated index |
| 108 | `vsl_ends` | `INTIN[0]`=begin, `INTIN[1]`=end | each `MIN..MAX_END_STYLE` else `DEF_END_STYLE`; `line_beg`/`line_end` | `INTOUT[0]`/`[1]` = accepted (squared/arrow/round) |
| 113 | `vsl_udsty` | `INTIN[0]` 16-bit mask | `ud_ls = INTIN[0]` (no validation) | none |

Notes: line widths are always **odd** (the line algorithm centres on a
pixel). End styles: 0 = squared, 1 = arrow, 2 = round.

## Marker attributes (`vdi_marker.c`)

| Op | Fn | Input | Validation → `Vwk` | Output |
|----|----|-------|--------------------|--------|
| 18 | `vsm_type`   | `INTIN[0]` type | `1..MAX_MARK_STYLE` else `DEF_MARK_STYLE`; `mark_index = v-1` | `INTOUT[0]` = accepted type |
| 19 | `vsm_height` | `PTSIN[1]` height | clamp `≥ DEF_MKHT` (and to the max marker size); sets `mark_height` + `mark_scale` | `PTSOUT[0]`/`[1]` = actual marker width/height |
| 20 | `vsm_color`  | `INTIN[0]` colour | `validate_color_index`; `mark_color = MAP_COL[i]` | `INTOUT[0]` = validated index |

Marker types (1..6): dot, plus, asterisk, circle, cross, square (the
device reports the count in `DEV_TAB[8]`). Marker height drives an
integer `mark_scale` of the base marker bitmap.

## NewTOS implementation notes

- Colours are stored as **PEL** (palette index) via `MAP_COL[]`; the
  framebuffer palette resolves PEL → RGB at draw time.
- `SIZ_TAB[6]` (max line width) and the marker size bounds come from
  the per-device tables (ADR-011), so different screens may clamp
  differently.
- `validate_color_index` clamps to the device's colour count
  (`DEV_TAB[13]`); index 1 maps to the default pen if out of range.
