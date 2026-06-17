# VDI `v_gdp` — generalized drawing primitives (Phase 00b D4)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_gdp.c` (`vdi_v_gdp`,
> `gdp_curve`, `gdp_rbox`), `vdi/vdi_text.c` (`gdp_justified`). Calling
> convention: [`../_dispatcher.md`](../_dispatcher.md). Opcode 11; the
> **sub-function is in `CONTRL[5]`**. Attributes from the
> [`Vwk`](../../structures/vwk.md). No output, no error return.

`v_gdp` is one dispatcher over `CONTRL[5]`:

| `CONTRL[5]` | Primitive | Input | Uses attrs |
|-------------|-----------|-------|------------|
| 1  | `v_bar`            | `PTSIN[0..3]` rect | fill + (perimeter if `fill_per`) |
| 2  | `v_arc`            | `PTSIN[0..1]` centre, `PTSIN[6]` radius; `INTIN[0..1]` begin/end angle | line |
| 3  | `v_pieslice`       | as arc | fill + line |
| 4  | `v_circle`         | `PTSIN[0..1]` centre, `PTSIN[4]` radius | fill |
| 5  | `v_ellipse`        | `PTSIN[0..1]` centre, `PTSIN[2]` xrad, `PTSIN[3]` yrad | fill |
| 6  | `v_ellarc`         | centre + `PTSIN[2/3]` radii; `INTIN[0..1]` angles | line |
| 7  | `v_ellpie`         | as ellarc | fill + line |
| 8  | `v_rbox`           | `PTSIN[0..3]` rect | line (rounded corners) |
| 9  | `v_rfbox`          | `PTSIN[0..3]` rect | fill (rounded corners) |
| 10 | `v_justified`      | see below | text |

(`CONTRL[5] == 13` = Bezier — out of bring-up scope.)

## Bar (1)

`vdi_vr_recfl(vwk)` fills the rectangle. Then, if `fill_per == TRUE`,
the four edges are drawn as a 5-point `polyline` in `fill_color` with
`LN_MASK = 0xffff` (solid). The edge coordinates are derived in place
from `PTSIN[0..3]`.

## Curves: arc / pieslice / circle / ellipse / ellarc / ellpie (2–7)

`gdp_curve`:
- Centre = `PTSIN[0..1]`.
- Radius: circle/arc/pieslice (≤4) use a single radius (`PTSIN[4]` for
  circle, `PTSIN[6]` for arc/pieslice) and derive `yrad` via the
  device aspect ratio (`mul_div(xrad, xsize, ysize)`); ellipse family
  (5–7) uses `PTSIN[2]`=xrad, `PTSIN[3]`=yrad. Negative radii are
  treated as positive.
- Angles (`INTIN[0]`=begin, `INTIN[1]`=end) in **tenths of a degree**
  (0..3600), CCW. Full shapes (circle 4, ellipse 5) force `0..TWOPI`.
  `del_ang = end-begin` (+TWOPI if negative).
- The arc is approximated by `clc_nsteps()` line segments (`clc_arc`).
- Early-out if the bounding box is fully outside the clip rect.
- **Pieslice/ellpie** close to the centre and fill; **arc/ellarc** are
  open polylines; **circle/ellipse** are closed and filled.

## Rounded boxes: rbox (8) / rfbox (9)

`gdp_rbox` from the rectangle `PTSIN[0..3]`. Corner radius is derived
from the rectangle size. `v_rbox` (8) outlines with the line attributes
(end styles forced `SQUARED`, then restored); `v_rfbox` (9) fills.

## Justified text (10)

`gdp_justified` (`vdi_text.c`):
- `CONTRL[3] - 2` = char count (`INTIN[0]`=interword flag,
  `INTIN[1]`=interchar flag, then the char codes).
- `PTSIN[0..1]` = position; `PTSIN[2]` = `max_x` (target run width).
- Distributes the slack `max_x - natural_width` across word gaps
  (interword) and/or all chars (interchar), honouring `chup`
  (0/900/1800/2700), then calls `output_text` with the per-char/word
  deltas (`JUSTINFO`). Interword expansion per gap is clamped to
  ±½ cell width.

## NewTOS implementation notes

- All sub-primitives reduce to the line/fill framebuffer primitives
  already used by [`_lines-markers.md`](_lines-markers.md) /
  [`_fill.md`](_fill.md) — `v_gdp` adds no new low-level op, only
  geometry generation. Target = `vwk->device` (ADR-011).
- Curve flattening (`clc_nsteps`/`clc_arc`) uses the device aspect
  (`xsize`/`ysize`); on a square-pixel framebuffer these are equal, so
  `yrad == xrad` for the circle case.
- Angle units match [`vst_rotation`](../attributes/_text.md) (tenths of
  a degree); reuse the same normalisation helper.
