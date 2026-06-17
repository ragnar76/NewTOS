# VDI attribute inquiries (Phase 00b D5)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_line.c` (`vdi_vql_attributes`),
> `vdi/vdi_marker.c` (`vdi_vqm_attributes`), `vdi/vdi_fill.c`
> (`vdi_vqf_attributes`), `vdi/vdi_text.c` (`vdi_vqt_attributes`).
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md). Each
> just reads the current [`Vwk`](../../structures/vwk.md) attribute
> state back out. No error return. Colours come back as **VDI index**
> via `REV_MAP_COL[]` (the inverse of `MAP_COL[]`). Several set
> `flip_y = 1` (PTSOUT in raster, not NDC).

## `vql_attributes` (35) — line

| Slot | Value |
|------|-------|
| `INTOUT[0]` | `line_index + 1` (1-based style) |
| `INTOUT[1]` | `REV_MAP_COL[line_color]` |
| `INTOUT[2]` | `wrt_mode + 1` |
| `PTSOUT[0]` | `line_width` |
| `PTSOUT[1]` | 0 |

## `vqm_attributes` (36) — marker

| Slot | Value |
|------|-------|
| `INTOUT[0]` | `mark_index` (**0-based** — note: *not* `+1`, unlike line/fill) |
| `INTOUT[1]` | `REV_MAP_COL[mark_color]` |
| `INTOUT[2]` | `wrt_mode + 1` |
| `PTSOUT[0]` | 0 |
| `PTSOUT[1]` | `mark_height` |

> ⚠ EmuTOS returns `mark_index` un-incremented here while
> `vsm_type` stores `type-1`. Preserve this exactly (a ported program
> may rely on it); flag in [`../_discrepancies.md`](../_discrepancies.md)
> if it ever conflicts with the Compendium. `flip_y = 1`.

## `vqf_attributes` (37) — fill

| Slot | Value |
|------|-------|
| `INTOUT[0]` | `fill_style` |
| `INTOUT[1]` | `REV_MAP_COL[fill_color]` |
| `INTOUT[2]` | `fill_index + 1` |
| `INTOUT[3]` | `wrt_mode + 1` |
| `INTOUT[4]` | `fill_per` |

## `vqt_attributes` (38) — text

| Slot | Value |
|------|-------|
| `INTOUT[0]` | `cur_font->font_id` |
| `INTOUT[1]` | `REV_MAP_COL[text_color]` |
| `INTOUT[2]` | `chup` (rotation, tenths°) |
| `INTOUT[3]` | `h_align` |
| `INTOUT[4]` | `v_align` |
| `INTOUT[5]` | `wrt_mode + 1` |
| `PTSOUT[0]` | `cur_font->max_char_width` |
| `PTSOUT[1]` | `cur_font->top` (char height) |
| `PTSOUT[2]` | `cur_font->max_cell_width` |
| `PTSOUT[3]` | `top + bottom + 1` (cell height, scaled-aware) |

`flip_y = 1`.

## NewTOS implementation notes

- These are pure `Vwk` reads — no device access. The only subtlety is
  the colour inverse map: NewTOS stores PEL in the `Vwk`, so `REV_MAP_COL`
  must round-trip exactly with the `MAP_COL` used by the setters.
- Keep the `vqm_attributes` 0-based quirk; it is the de-facto TOS ABI.
- `flip_y` is EmuTOS's NDC/RC Y-axis flag; for NewTOS the screen
  workstation is RC (Y-down) so PTSOUT is already device coords —
  carry the flag only if/when an NDC workstation is added.
