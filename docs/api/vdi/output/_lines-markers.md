# VDI polyline & polymarker output primitives (Phase 00b D4)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_line.c:1000` (`vdi_v_pline`),
> `vdi/vdi_marker.c:86` (`vdi_v_pmarker`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). Both consume `PTSIN`
> coordinate pairs; **no output array, no error return**. Attribute state
> comes from the [`Vwk`](../../structures/vwk.md), set via the
> [line/marker attribute setters](../attributes/_line-marker.md) and
> summarised in [`../_state.md`](../_state.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** opcodes (6 / 7); the `PTSIN` vertex contract (count in
> `CONTRL[1]`, `(x,y)` pairs in raster coordinates); which current
> attributes are consulted; the `MAX_VERTICES` clamp; the "no output"
> shape. **Reference:** the Bresenham/`polyline`/`wideline`/`arrow` pixel
> loops, the Cohen–Sutherland `clip_line`, the `LINE_STYLE[]`/`LN_MASK`
> dash machinery, and the built-in marker glyph vector tables — NewTOS
> draws its own way (`tos-api/src/vdi/backend.rs`).

| Op | Fn | Input | Draws |
|----|----|-------|-------|
| 6 | `v_pline`   | `CONTRL[1]` = vertex count; `PTSIN[]` = `(x,y)` pairs | connected polyline through the points, in the current line attributes |
| 7 | `v_pmarker` | `CONTRL[1]` = point count; `PTSIN[]` = `(x,y)` pairs | one marker glyph centred at each point, in the current marker attributes |

Native ABI (both): GEM trap `svc #2`, selector 115 in `x0`, `VDIPB*` in
`x1`; opcode in `CONTRL[0]`, handle in `CONTRL[6]`, vertex count in
`CONTRL[1]`, coordinates in `PTSIN[]`. The dispatcher pre-loads
`CONTRL[2]=CONTRL[4]=0` (no outputs) for both (`vdi_main.c:32-72`,
jump-table `{nptsout,nintout} = {0,0}`).

---

## `v_pline` — polyline (opcode 6)

### EmuTOS source

**Primary location:** `vdi/vdi_line.c:1000` — `vdi_v_pline(Vwk *vwk)`
(reached via `jmptb1[5]`, `vdi_main.c:37`).

**Helpers called (reference):**
- `vdi_line.c:2384` — `set_LN_MASK()` — loads `LN_MASK` from
  `LINE_STYLE[line_index]` (or `ud_ls` when `line_index >= 6`).
- `vdi_line.c:1101` — `polyline()` — width-1 path; clips each segment via
  `clip_line()` and calls `abline()` (Bresenham).
- `vdi_line.c:1389` — `wideline()` — width > 1 path.
- `vdi_line.c:1597` — `arrow()` — draws arrowheads when an end is `ARROWED`.
- `vdi_line.c:1057` — `clip_line()` — Cohen–Sutherland segment clip.

**Globals read (attribute state, from `Vwk`):** `line_index` (dash style),
`ud_ls` (user dash mask), `line_width`, `line_color`, `line_beg`/`line_end`
(end styles), `wrt_mode`, `clip` + `xmn_clip`/`ymn_clip`/`xmx_clip`/
`ymx_clip`. Reads `CONTRL[1]` (vertex count) and `PTSIN[]`.

**Globals written (reference):** `LN_MASK`, `LSTLIN` (Line-A dash state).

**Build configuration:** `HAVE_BEZIER` is **hardcoded 0** in
`vdi_defs.h:18` ("entirely untested"), so the `CONTRL[5]==13` Bezier
sub-case (`vdi_line.c:1007-1010`) is compiled out — `v_pline` always takes
the polyline/wideline path in the default build.

### Behaviour

`vdi_v_pline` treats `PTSIN` as `CONTRL[1]` `(x,y)` vertices (raster
coordinates, origin top-left) and draws the open polyline connecting them
in order. It first loads the dash mask (`set_LN_MASK`), then:

- if `line_width == 1`: `polyline()` strokes the segments in `line_color`;
  if either `line_beg` or `line_end` has the `ARROWED` bit (1), `arrow()`
  adds arrowheads at the corresponding end(s) (`vdi_line.c:1014-1017`);
- else (`line_width > 1`): `wideline()` draws the thick line (line widths
  are always odd — see [`vsl_width`](../attributes/_line-marker.md)).

Each segment is clipped to the `Vwk` clip rectangle (`clip_line`,
Cohen–Sutherland) **only when `vwk->clip` is set**; with clipping off,
segments are drawn unclipped (`vdi_line.c:1115-1116`). `wrt_mode` selects
the raster combination (replace/transparent/xor/erase).

### PTSIN vertex contract

- `CONTRL[1]` = number of vertices (= number of `(x,y)` pairs).
- `PTSIN[2i] = x_i`, `PTSIN[2i+1] = y_i`, in raster coordinates.
- The polyline is **open** — no automatic close. A caller that wants a
  closed outline repeats the first vertex as the last.
- Count `<= 1`: `polyline()`'s loop runs from `count-1` downward and does
  nothing for a single point (`vdi_line.c:1106`), so a 0/1-vertex polyline
  draws nothing (no error).

### Inputs

| Slot | Name | Meaning |
|------|------|---------|
| `CONTRL[1]` | vertex count | number of `(x,y)` pairs (clamped to `MAX_VERTICES` by the dispatcher) |
| `PTSIN[2i]`, `PTSIN[2i+1]` | `x_i`, `y_i` | vertex `i`, raster coordinates |

### Outputs

**None.** No `INTOUT`, no `PTSOUT` (`{nptsout,nintout}={0,0}`). VDI has no
error return; the only "failure" is a silent no-op (invalid handle in the
dispatcher, or a fully-clipped line).

---

## `v_pmarker` — polymarker (opcode 7)

### EmuTOS source

**Primary location:** `vdi/vdi_marker.c:86` — `vdi_v_pmarker(Vwk *vwk)`
(reached via `jmptb1[6]`, `vdi_main.c:38`).

**Helpers called (reference):** `vdi_v_pline()` — each marker is emitted as
a set of short polylines (`vdi_marker.c:146`).

**Globals read (attribute state, from `Vwk`):** `mark_index` (0..5,
glyph selector), `mark_scale` (integer scale from `vsm_height`),
`mark_color`, plus the clip rect. Reads `CONTRL[1]` (point count) and
`PTSIN[]`.

**Globals written:** temporarily overwrites `vwk->line_index`/`line_color`/
`line_width`/`line_beg`/`line_end`/`clip` and the global `PTSIN` pointer +
`CONTRL[1]` while drawing; restores the line attributes and `PTSIN` on exit
(`vdi_marker.c:101-161`). **See edge case: `clip` is NOT restored.**

**Marker glyph tables (reference):** `m_dot`/`m_plus`/`m_star`/`m_square`/
`m_cross`/`m_dmnd` (`vdi_marker.c:18-23`) — const `WORD` vector arrays,
each `{num_polylines, {num_points, x,y, …}, …}`.

**Build configuration:** none.

### Behaviour

For each of the `CONTRL[1]` input points, `v_pmarker` draws the marker
glyph selected by `mark_index` (0..5), scaled by `mark_scale` and centred
on that point. Each glyph is defined as one or more short polylines of
integer offsets from the centre; the offsets are multiplied by
`mark_scale` and added to the point's `(x,y)`, then drawn via
`vdi_v_pline`.

Markers are drawn with **forced** line attributes, not the caller's:
`line_index = 0` (solid), `line_color = mark_color`, `line_width = 1`,
`line_beg = line_end = SQUARED` (no arrowheads), and `clip = 1` (clipping
forced on) — `vdi_marker.c:108-114`. The caller's line attributes are
saved and restored around the whole call so `v_pmarker` never disturbs
them; **`clip` is not saved/restored** (see edge cases).

| `mark_index` | glyph | table | # polylines |
|:---:|---------|------------|:---:|
| 0 | dot      | `m_dot`    | 1 (2-point degenerate) |
| 1 | plus     | `m_plus`   | 2 |
| 2 | asterisk | `m_star`   | 3 |
| 3 | square   | `m_square` | 1 (5-point) |
| 4 | cross    | `m_cross`  | 2 |
| 5 | diamond  | `m_dmnd`   | 1 (5-point) |

`mark_index` is stored 0-based by `vsm_type` (op 18: `mark_index = type-1`
for `type` 1..6, else default 3 → index 2 = asterisk); see
[`../_state.md`](../_state.md).

### PTSIN vertex contract

- `CONTRL[1]` = number of marker centres (= `(x,y)` pairs).
- `PTSIN[2i]`, `PTSIN[2i+1]` = centre of marker `i`, raster coordinates.
- Each centre is independent; there is no connecting line between markers.

### Inputs

| Slot | Name | Meaning |
|------|------|---------|
| `CONTRL[1]` | point count | number of marker centres (clamped to `MAX_VERTICES` by the dispatcher) |
| `PTSIN[2i]`, `PTSIN[2i+1]` | `x_i`, `y_i` | marker-`i` centre |

### Outputs

**None** (`{nptsout,nintout}={0,0}`).

---

## Verification criteria

- **`v_pline` nominal:** `CONTRL[1]=2`, `PTSIN=[x0,y0,x1,y1]` → a single
  line segment from `(x0,y0)` to `(x1,y1)` in `line_color`, dashed per
  `line_index`, thickness `line_width`, clipped to the `Vwk` clip rect
  when `clip` is on.
- **`v_pline` open, not closed:** `CONTRL[1]=3` with three distinct points
  draws two segments (1→2, 2→3), **not** a closed triangle (the caller
  must repeat the first vertex to close).
- **`v_pline` arrowheads:** with `line_end` = `ARROWED` (1) and
  `line_width == 1`, an arrowhead is drawn at the last vertex; with
  `SQUARED` ends, none.
- **`v_pline` degenerate:** `CONTRL[1]` of 0 or 1 draws nothing (no error).
- **`v_pmarker` nominal:** `CONTRL[1]=1`, `PTSIN=[x,y]`, `mark_index=2`
  (asterisk), `mark_scale=1` → the asterisk glyph centred at `(x,y)` in
  `mark_color`, solid width-1 lines.
- **`v_pmarker` scaling:** `mark_scale=2` doubles every glyph offset;
  `mark_index=0` (dot) is a degenerate 1-pixel marker regardless of scale.
- **`v_pmarker` isolation:** the caller's `line_index`/`line_color`/
  `line_width`/`line_beg`/`line_end` are unchanged after the call.
- **`MAX_VERTICES` clamp (both):** passing more than `MAX_VERTICES`
  vertices draws only the first `MAX_VERTICES` (dispatcher-level, see
  [`../_dispatcher.md`](../_dispatcher.md)); `CONTRL[1]` is restored on exit.

---

## Edge cases discovered in source

1. **`v_pmarker` leaves `clip` forced on.** It sets `vwk->clip = 1`
   (`vdi_marker.c:114`) but the restore block (`:157-161`) only restores
   the five line attributes — **not `clip`**. If the workstation had
   clipping *disabled*, a `v_pmarker` call silently **enables** it for all
   subsequent drawing until the program next calls `vs_clip`. Preserved
   verbatim as the source behaviour (see [`../_discrepancies.md`](../_discrepancies.md)
   D-VDI-9).
2. **`v_pmarker` reuses `PTSIN` in place.** It swaps the global `PTSIN`
   pointer to a 10-word scratch buffer (`sav_points`) and overwrites
   `CONTRL[1]` per glyph polyline, restoring `PTSIN` (but not `CONTRL[1]`
   to the original point count) at the end (`vdi_marker.c:119-154`). A
   pure-reference reuse detail — NewTOS regenerates glyph geometry
   directly, so it does not apply.
3. **`v_pline` selects the dash mask from `line_index`**, using
   `LINE_STYLE[line_index]` for `line_index < 6` and the user mask `ud_ls`
   for `line_index >= 6` (`set_LN_MASK`, `vdi_line.c:2388-2389`). Line
   type 7 (`vsl_type(7)` → `line_index=6`) therefore draws with the
   `vsl_udsty` user pattern.
4. **Clipping is only applied when `vwk->clip` is set.** With clipping off,
   `polyline` calls `abline` directly on the raw coordinates
   (`vdi_line.c:1115-1119`), so out-of-bounds coordinates reach the raster
   layer unclamped — the backend must guard the framebuffer bounds itself.
5. **`v_pmarker` never uses the caller's `wrt_mode` differently** — it
   still reads `wrt_mode` through the forced-attribute `polyline` path, so
   markers honour the current writing mode even though the line style/colour
   are overridden.

---

## Dependencies

### Requires
- An open workstation (`CONTRL[6]` resolved to a live `Vwk` by the
  dispatcher; a bad handle is a silent no-op).
- The line attribute state (`line_index`/`ud_ls`/`line_color`/`line_width`/
  `line_beg`/`line_end`) for `v_pline`; the marker state
  (`mark_index`/`mark_scale`/`mark_color`) for `v_pmarker` — see
  [`../attributes/_line-marker.md`](../attributes/_line-marker.md).
- `wrt_mode` and the clip rectangle (`vswr_mode` op 32, `vs_clip` op 129).

### Used by
- `v_pmarker` calls `vdi_v_pline` internally.
- `gdp_curve` (arc/ellarc, [`_gdp.md`](_gdp.md)) calls `polyline`/
  `wideline` for open curves; `polygon()` uses `polyline` for the
  perimeter of filled shapes.

---

## Related data structures

- [`../../structures/vwk.md`](../../structures/vwk.md) — the `line_*` /
  `mark_*` / clip fields.
- [`../_state.md`](../_state.md) — attribute-state defaults and valid ranges.
- Marker constants: `MIN_MARK_STYLE`=1, `MAX_MARK_STYLE`=6,
  `DEF_MARK_STYLE`=3 (`vdi_defs.h:55-57`); end styles `SQUARED`=0,
  `ARROWED`=1, `ROUND`=2 (`vdi_defs.h:89-91`).

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the six marker
  glyph shapes, the "polyline is open (not auto-closed)" rule, and the
  `v_pmarker`-forces-`clip` behaviour is **deferred**.

## Configuration / version notes

- `HAVE_BEZIER` = 0 (`vdi_defs.h:18`) — the `v_pline` Bezier sub-case
  (`CONTRL[5]==13`) is compiled out in every default build. If a program
  passes sub-op 13, `v_pline` treats it as an ordinary polyline.
- Otherwise configuration-independent. `MAX_VERTICES` (the shared vertex
  cap) is a build-config value — 512 (small/STe) or 1024 (default) — see
  [`../_dispatcher.md`](../_dispatcher.md) and `_discrepancies.md` D-VDI-3.

## NewTOS implementation notes (Phase 12)

- `polyline`/`wideline`/`arrow` map to the framebuffer line primitive
  (Bresenham), honouring `wrt_mode` and the dash `LN_MASK`. Target
  framebuffer = `vwk->device` (ADR-011), not a global `v_bas_ad`.
- The marker glyph tables (`m_dot` … `m_dmnd`) are small const vector
  arrays that can be transcribed verbatim (they are data, not algorithm);
  NewTOS generates the marker polylines from them.
- Decide whether `v_pmarker`'s clip-forcing side effect is reproduced
  (D-VDI-9). Bit-exact compatibility argues for keeping it; document the
  choice.
