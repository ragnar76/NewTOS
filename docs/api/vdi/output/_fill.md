# VDI area-fill primitives (Phase 00b D4)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_fill.c` (`vdi_v_fillarea`,
> `vdi_vr_recfl`, `vdi_v_contourfill`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). Fill attributes from the
> [`Vwk`](../../structures/vwk.md) (set via the
> [fill setters](../attributes/_fill.md)). No output, no error return.

| Op | Fn | Input | Fills |
|----|----|-------|-------|
| 9 | `v_fillarea`    | `CONTRL[1]` = vertex count; `PTSIN[]` = x,y pairs | the closed polygon |
| 114 | `vr_recfl`    | `PTSIN[0..3]` = rectangle corners | the rectangle (no perimeter) |
| 103 | `v_contourfill` | `PTSIN[0..1]` = seed x,y; `INTIN[0]` = boundary colour (or −1) | flood-fill region around the seed |

## `v_fillarea` (9)

`polygon(vwk, PTSIN, CONTRL[1])` — scan-converts the closed polygon and
fills each scanline span with the current interior style/pattern in
`fill_color`, clipped to the `Vwk` clip rect. If `fill_per` is set, the
perimeter is outlined with the line attributes afterwards.
(Bezier-fill sub-case `CONTRL[5]==13` is out of bring-up scope.)

## `vr_recfl` (114)

1. `arb_corner(PTSIN)` — normalise the two corners to (lower-left,
   upper-right) order.
2. Copy to a temp `Rect` (so clipping doesn't corrupt `PTSIN`, which a
   perimeter pass — e.g. GDP bar — may still need).
3. If `clip`, `clipbox()` the rect; quit if fully clipped.
4. `draw_rect(vwk, &rect, fill_color)` with the current fill pattern.

> `vr_recfl` itself draws **no** perimeter; the GDP `v_bar`
> ([`_gdp.md`](_gdp.md) sub-op 1) adds the outline when `fill_per`.

## `v_contourfill` (103)

Seed flood-fill (`contourfill`) starting at `PTSIN[0..1]`:
- Reads the seed pixel's colour. The fill spreads to all connected
  pixels matching the seed colour, stopping at:
  - `INTIN[0] >= 0`: pixels of that explicit **boundary colour**.
  - `INTIN[0] < 0`: any pixel **not** equal to the seed colour
    (fill-while-same).
- Fills with the current interior style in `fill_color`, clipped to the
  `Vwk` clip rect. `SEEDABORT` (`no_abort`) is the abort hook.

## NewTOS implementation notes

- `polygon`/`draw_rect`/`contourfill` map to the framebuffer span-fill
  primitive, ANDing the active pattern (`patptr`/`patmsk`,
  [`vsf_*`](../attributes/_fill.md)) per scanline and honouring
  `wrt_mode`. Target = `vwk->device` (ADR-011).
- Flood-fill reads pixels back via the framebuffer pixel-read path
  (palette index for the bring-up screen); needs a scanline seed stack —
  size it to the device width to bound recursion.
- `REV_MAP_COL`/`MAP_COL` translate between VDI colour index and PEL as
  elsewhere.
