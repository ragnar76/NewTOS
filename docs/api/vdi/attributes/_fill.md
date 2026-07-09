# VDI fill attribute setters (Phase 00b D3)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_fill.c`; limits in
> `vdi/vdi_defs.h:59-74`. Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). Same setter shape as
> [`_line-marker.md`](_line-marker.md): validate → store in `Vwk` → echo the
> accepted value. No error return.
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** opcode numbers, the input slot, the clamp/default rules and
> the echoed value, and the interior-style / pattern-index constant ranges.
> **Reference:** the `Vwk` fields, the ROM dither/hatch pattern tables, and
> `st_fl_ptr()` which selects the active `patptr`/`patmsk`.

## Trap interface (all setters below)

Native ABI: GEM trap `svc #2`, selector 115 in `x0`, `VDIPB*` in `x1`.
Per function: opcode in `CONTRL[0]`, handle in `CONTRL[6]`. Inputs/outputs
per the table below.

| Op | Fn | Reads | Validation → `Vwk` | Echoes |
|----|----|-------|--------------------|--------|
| 23 | `vsf_interior`  | `INTIN[0]` interior style | `MIN_FILL_STYLE..MAX_FILL_STYLE` (0..4) else `DEF_FILL_STYLE` (0=hollow); `fill_style`; then `st_fl_ptr()` | `INTOUT[0]` = accepted style |
| 24 | `vsf_style`     | `INTIN[0]` pattern index | if `fill_style == FIS_PATTERN`: clamp `1..MAX_FILL_PATTERN` (24) else `DEF_FILL_PATTERN` (1); **otherwise** clamp `1..MAX_FILL_HATCH` (12) else `DEF_FILL_HATCH` (1); `fill_index = accepted-1` (**0-based**); then `st_fl_ptr()` | `INTOUT[0]` = accepted index (1-based) |
| 25 | `vsf_color`     | `INTIN[0]` colour | `validate_color_index`; `fill_color = MAP_COL[i]` | `INTOUT[0]` = validated index |
| 104 | `vsf_perimeter` | `INTIN[0]` flag | `0` → `fill_per = FALSE`; ≠0 → `fill_per = TRUE` | `INTOUT[0]` = 0 / 1 |
| 112 | `vsf_udpat`    | `INTIN[0..]` pattern data | copy `CONTRL[3]` WORDs into `ud_patrn[]`; see buffer semantics below | none |

Source: `vdi_fill.c:173` (`vsf_interior`), `:186` (`vsf_style`), `:206`
(`vsf_color`), `:219` (`vsf_perimeter`), `:149` (`vsf_udpat`).

### Value constants (`vdi_defs.h`)

| `#define` | Value | Meaning |
|-----------|-------|---------|
| `MIN_FILL_STYLE` / `MAX_FILL_STYLE` / `DEF_FILL_STYLE` | 0 / 4 / 0 (`FIS_HOLLOW`) | interior-style range + default |
| `FIS_HOLLOW`/`FIS_SOLID`/`FIS_PATTERN`/`FIS_HATCH`/`FIS_USER` | 0 / 1 / 2 / 3 / 4 | interior styles |
| `MIN_FILL_PATTERN` / `MAX_FILL_PATTERN` / `DEF_FILL_PATTERN` | 1 / 24 / 1 | dither-pattern index range (when style = PATTERN) |
| `MIN_FILL_HATCH` / `MAX_FILL_HATCH` / `DEF_FILL_HATCH` | 1 / 12 / 1 | hatch index range (for every other style) |

### Interior styles (`fill_style`, `vsf_interior`)

| Value | `FIS_*` | Meaning |
|-------|---------|---------|
| 0 | HOLLOW | no fill (background only) |
| 1 | SOLID | solid `fill_color` |
| 2 | PATTERN | one of `MAX_FILL_PATTERN` (24) dither patterns, selected by `vsf_style` |
| 3 | HATCH | one of `MAX_FILL_HATCH` (12) cross-hatch patterns, selected by `vsf_style` |
| 4 | USER | the `vsf_udpat` user-defined pattern |

### `vsf_style` — index is re-interpreted against the *current* interior style

`vsf_style` validates `INTIN[0]` using the pattern range **only when
`fill_style == FIS_PATTERN`**; for **every other** interior style
(hollow/solid/hatch/user) it uses the *hatch* range `1..12`
(`vdi_fill.c:192-198`). So the accepted value and the effective pattern depend
on the interior style set by a *prior* `vsf_interior` call — set
`vsf_interior` first. The accepted 1-based value is echoed in `INTOUT[0]`, but
`fill_index` is stored **0-based** (`accepted-1`, `vdi_fill.c:199`).

### `vsf_udpat` — user fill pattern buffer semantics (`vdi_fill.c:149-166`)

- `count = CONTRL[3]` (the `INTIN` word count).
- **`count == 16`** → single-plane pattern; `multifill = 0`.
- **`count == INQ_TAB[4] * 16`** (i.e. 16 words per plane, `INQ_TAB[4]` =
  number of planes) → valid multi-plane coloured pattern; `multifill = 1`.
- **Any other count** → **invalid: return immediately, nothing stored, no
  state change** (`vdi_fill.c:159-160`).
- On a valid count, `count` WORDs are copied from `INTIN` into `ud_patrn[]`.
- No `INTOUT`/`PTSOUT` (jump-table counts are 0/0).

Selecting the user pattern for drawing still requires
`vsf_interior(FIS_USER)`; `vsf_udpat` only fills the buffer.

---

## Verification criteria

- **`vsf_interior` clamp:** `vsf_interior(7)` (out of `0..4`) → `INTOUT[0]=0`
  (`FIS_HOLLOW`); `vsf_interior(2)` → `INTOUT[0]=2`, `fill_style=FIS_PATTERN`.
- **`vsf_style` pattern branch:** after `vsf_interior(FIS_PATTERN)`,
  `vsf_style(30)` → `INTOUT[0]=1` (`DEF_FILL_PATTERN`, out of `1..24`),
  `fill_index=0`; `vsf_style(24)` → `INTOUT[0]=24`, `fill_index=23`.
- **`vsf_style` hatch branch:** after `vsf_interior(FIS_HATCH)` (or any
  non-PATTERN style), `vsf_style(20)` → `INTOUT[0]=1` (`DEF_FILL_HATCH`,
  out of `1..12`); `vsf_style(12)` → `INTOUT[0]=12`, `fill_index=11`.
- **`vsf_color`:** `vsf_color(1)` → `INTOUT[0]=1`,
  `fill_color = MAP_COL[1] = numcolors-1`; out-of-range index echoes 1.
- **`vsf_perimeter`:** `vsf_perimeter(0)` → `INTOUT[0]=0`, `fill_per=FALSE`;
  `vsf_perimeter(5)` → `INTOUT[0]=1`, `fill_per=TRUE`.
- **`vsf_udpat` single-plane:** `CONTRL[3]=16` → `multifill=0`, 16 words
  copied. **Multi-plane:** `CONTRL[3]=INQ_TAB[4]*16` → `multifill=1`.
  **Invalid:** `CONTRL[3]=10` → no change to `ud_patrn`/`multifill`.

---

## Edge cases discovered in source

1. **`vsf_style` uses the hatch range for hollow/solid/user too**, not only
   for hatch (`vdi_fill.c:192-198`). A `vsf_style` call while
   `fill_style != FIS_PATTERN` always clamps to `1..12`.
2. **`fill_index` is stored 0-based** (`accepted-1`) but echoed 1-based —
   and `vqf_attributes` (op 37) returns `fill_index+1` (`vdi_fill.c:328`),
   so the *inquiry* round-trips correctly, unlike the marker case.
3. **Interior-style default `FIS_HOLLOW`, but the perimeter default is
   drawn** — `init_wk` sets `fill_per = TRUE` (`vdi_control.c:325`), so a
   freshly opened workstation outlines its (hollow) fills unless
   `vsf_perimeter(0)` is called.
4. **`vsf_udpat` silently drops an invalid word count** — no clamp, no
   default; it just returns (`vdi_fill.c:159-160`).
5. **`init_wk` stores `fill_index` 1-based, the setter stores it 0-based
   (D-VDI-8).** `init_wk` (`vdi_control.c:311`) sets `vwk->fill_index = l`
   (the validated 1-based value), while `vsf_style` stores `fi-1`. For the
   same requested pattern the open-time default therefore ends up one higher
   than a later `vsf_style` would set. Preserved verbatim; see
   [`../_discrepancies.md`](../_discrepancies.md) D-VDI-8.
6. **`vsf_interior` never resets or re-validates `fill_index` when the
   style changes — `st_fl_ptr()` can read past the end of `HATCH1[96]`
   as a result (out-of-bounds, EmuTOS source-level gap).**
   `vdi_vsf_interior` (`vdi_fill.c:173-181`) only stores `fill_style` and
   calls `st_fl_ptr()`; it leaves the *existing* `fill_index` untouched.
   `st_fl_ptr()` (`vdi_fill.c:338-380`) re-interprets that `fill_index`
   under the *new* `fill_style` with no bounds check: for `FIS_PATTERN`,
   `fill_index` `0..23` is safe (`DITHER[32]` for `fi<8`, `OEMPAT[128]`
   for `fi>=8`); for `FIS_HATCH`, only `fill_index` `0..11` is safe
   (`HATCH0[48]` for `fi<6`, `HATCH1[96]` for `fi>=6`, `(fi-6)*16` must
   stay `<96`). Calling `vsf_interior(FIS_PATTERN)` →
   `vsf_style(<13..24>)` (stores `fill_index` `12..23`) →
   `vsf_interior(FIS_HATCH)` **without an intervening `vsf_style`**
   leaves `fill_style=FIS_HATCH` with the stale `fill_index` in
   `12..23`; `st_fl_ptr()`'s `FIS_HATCH` branch then computes
   `&HATCH1[(fill_index-6)*16]`, up to `HATCH1[272]` — **176 words past
   the 96-word array** — and `vwk->patptr` ends up pointing at whatever
   static data trails `HATCH1[]` in the binary. The next fill/GDP draw
   reads that as a 16-word hatch pattern. (The reverse direction,
   hatch→pattern, stays in-bounds: pattern's own tables cover the full
   `0..23` range.) Unlike this, the line-style table is explicitly
   guarded (`line_index < 6` before indexing `LINE_STYLE[]`, else
   `ud_ls`, `vdi_line.c:2389`) — fill has no equivalent guard. See
   [`../_discrepancies.md`](../_discrepancies.md) D-081 (global log).

---

## Dependencies

### Requires
- An open workstation; `INQ_TAB[4]` (number of planes) for the `vsf_udpat`
  multi-plane count; `numcolors`/`MAP_COL[]` for `vsf_color`.
- `st_fl_ptr()` (`vdi_fill.c:338`) — recomputes the active `patptr`/`patmsk`
  from `fill_style`/`fill_index` after `vsf_interior`/`vsf_style`. It does
  **not** re-validate `fill_index` against the new `fill_style`'s range —
  see edge case 6 above (D-081) for the resulting out-of-bounds read.

### Used by
- `v_fillarea` (9), `v_bar`/other GDPs (11), `v_contourfill` (103),
  `vr_recfl` (114) — all consult `fill_style`/`fill_index`/`fill_color`/
  `fill_per` and the `patptr`/`patmsk` set by `st_fl_ptr`.
- `vqf_attributes` (37).

---

## Related data structures

- [`../../structures/vwk.md`](../../structures/vwk.md) — `fill_*`,
  `ud_patrn[]`, `multifill`, `patptr`/`patmsk` fields.
- [`../_state.md`](../_state.md) — attribute-state defaults table.

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the pattern/hatch
  index ranges, the `vsf_style`-uses-hatch-range behaviour, and the
  `vsf_udpat` count rule is **deferred**.

## Configuration / version notes

- `UDPAT_PLANES` (size of `ud_patrn[]`) is **4** in the default build, 8 with
  `CONF_WITH_TT_SHIFTER`, 32 with `CONF_WITH_VIDEL` (`vdi_defs.h:25-31`) —
  both shifters **0** by default. The user pattern therefore holds up to
  `4*16` WORDs (default). `vsf_udpat`'s multi-plane count uses the *live*
  plane count `INQ_TAB[4]`, not `UDPAT_PLANES`.
- Otherwise configuration-independent.

## NewTOS implementation notes (Phase 12)

- The dither (`MAX_FILL_PATTERN`=24) and hatch (`MAX_FILL_HATCH`=12) tables
  are the standard GEM patterns (`vdi_fill.c` ROM tables `HATCH0`/`HATCH1`/
  the DITHER/OEMPAT arrays); the framebuffer fill primitive ANDs the pattern
  mask per scanline against `fill_color` (honouring `wrt_mode` for the
  background).
- `fill_per` (perimeter) draws the outline with the *line* attributes after
  the area fill — `v_fillarea`/`v_bar` honour it (default TRUE at open).
- For the 640×480×16-colour (4-plane) framebuffer, `INQ_TAB[4] = 4`, so a
  valid multi-plane `vsf_udpat` count is `64` WORDs — confirm during
  backend bring-up (`../_discrepancies.md` D-VDI-7).
