# VDI polyline & polymarker primitives (Phase 00b D4)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_line.c` (`vdi_v_pline`),
> `vdi/vdi_marker.c` (`vdi_v_pmarker`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). Both consume `PTSIN`
> coordinate pairs; no output, no error return. Attribute state comes
> from the [`Vwk`](../../structures/vwk.md) (set via
> [attribute setters](../attributes/_line-marker.md)).

| Op | Fn | Input | Draws |
|----|----|-------|-------|
| 6 | `v_pline`   | `CONTRL[1]` = vertex count; `PTSIN[]` = x,y pairs | connected polyline through the points |
| 7 | `v_pmarker` | `CONTRL[1]` = point count; `PTSIN[]` = x,y pairs | one marker glyph at each point |

## `v_pline` (6)

1. `set_LN_MASK(vwk)` — load the dash bitmask from `line_index`
   (`ud_ls` if user style).
2. If `line_width == 1`: `polyline()` in `line_color`; then if
   `line_beg | line_end` has the `ARROWED` bit, draw arrowheads
   (`arrow()`).
3. Else (`line_width > 1`): `wideline()` — the thick-line path; widths
   are always odd (see [`vsl_width`](../attributes/_line-marker.md)).
4. Each segment is clipped to the `Vwk` clip rect (`clip_line`,
   Cohen–Sutherland) when `clip` is on.

> Bezier sub-case (`CONTRL[5] == 13`, `HAVE_BEZIER`) is out of bring-up
> scope (see working list); the polyline path is the default.

## `v_pmarker` (7)

For each input point, the marker is drawn as a set of **short
polylines** from a built-in table indexed by `mark_index` (0..5):

| `mark_index` | glyph | table |
|--------------|-------|-------|
| 0 | dot     | `m_dot`    |
| 1 | plus    | `m_plus`   |
| 2 | asterisk| `m_star`   |
| 3 | square  | `m_square` |
| 4 | cross   | `m_cross`  |
| 5 | diamond | `m_dmnd`   |

- The marker's vector points are scaled by `mark_scale`
  (from [`vsm_height`](../attributes/_line-marker.md)) and offset to
  each centre.
- Drawn with **forced** line attributes: `line_index = 0` (solid),
  `line_width = 1`, square ends, `line_color = mark_color`, and clipping
  **forced on**. The caller's line attributes are saved and restored
  around the call (so `v_pmarker` never disturbs them).

## NewTOS implementation notes

- `polyline`/`wideline`/`arrow` map to the framebuffer line primitive
  (Bresenham), honouring `wrt_mode` and the dash `LN_MASK`. Target
  framebuffer = `vwk->device` (ADR-011), not a global `v_bas_ad`.
- The marker glyph tables (`m_dot` … `m_dmnd`) are small const vector
  arrays ported verbatim from `vdi_marker.c`.
- Dot (`mark_index == 0`) is a degenerate 1-pixel marker; all others
  scale with `mark_scale`.
