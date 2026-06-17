# VDI fill attribute setters (Phase 00b D3)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_fill.c`. Same setter shape as
> [`_line-marker.md`](_line-marker.md): validate → store in `Vwk` →
> echo accepted value. No error return.

| Op | Fn | Input | Validation → `Vwk` | Output |
|----|----|-------|--------------------|--------|
| 23 | `vsf_interior`  | `INTIN[0]` interior style | `MIN..MAX_FILL_STYLE` else `DEF_FILL_STYLE`; `fill_style` | `INTOUT[0]` = accepted |
| 24 | `vsf_style`     | `INTIN[0]` pattern index | if `fill_style == FIS_PATTERN`: `1..MAX_FILL_PATTERN`; else (hatch) `1..MAX_FILL_HATCH`; else default; `fill_index` | `INTOUT[0]` = accepted |
| 25 | `vsf_color`     | `INTIN[0]` colour | `validate_color_index`; `fill_color = MAP_COL[i]` | `INTOUT[0]` = validated index |
| 104 | `vsf_perimeter` | `INTIN[0]` flag | `0` → `fill_per = FALSE`; ≠0 → `TRUE` | `INTOUT[0]` = 0 / 1 |
| 112 | `vsf_udpat`    | `INTIN[0..]` pattern data | `CONTRL[3]` = 16 (single plane) or `16*planes`; copied into `ud_patrn[]`; sets `multifill` for multi-plane | none |

### Interior styles (`fill_style`)

| Value | `FIS_*` | Meaning |
|-------|---------|---------|
| 0 | HOLLOW | no fill (background only) |
| 1 | SOLID | solid `fill_color` |
| 2 | PATTERN | one of `MAX_FILL_PATTERN` dither patterns (index via `vsf_style`) |
| 3 | HATCH | one of `MAX_FILL_HATCH` cross-hatch patterns |
| 4 | USER | the `vsf_udpat` user-defined pattern |

`vsf_style` reinterprets `INTIN[0]` against the **current** interior
style (pattern vs hatch ranges differ), so set `vsf_interior` first.
`vsf_udpat` with a 16-word count is a single-plane pattern; a
`16*planes`-word count is a coloured multi-plane pattern (`multifill`).

## NewTOS implementation notes

- The dither/hatch pattern tables are the standard GEM patterns
  (`vdi_fill.c` ROM tables); the framebuffer fill primitive ANDs the
  pattern mask per scanline against `fill_color` (and the background
  for HOLLOW/transparent modes per `wrt_mode`).
- `fill_per` (perimeter) draws the outline with the *line* attributes
  after the area fill — `v_fillarea`/`v_bar` honour it.
