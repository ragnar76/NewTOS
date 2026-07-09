# VDI line & marker attribute setters (Phase 00b D3)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_line.c` (`vsl_*`),
> `vdi/vdi_marker.c` (`vsm_*`), limits in `vdi/vdi_defs.h:45-57,88-106`.
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** the opcode numbers, the `INTIN`/`PTSIN` slot each setter
> reads, the clamp/default rules that turn the *requested* value into the
> *actually selected* value, and the value echoed back in `INTOUT`/`PTSOUT`.
> **Reference:** the storage in `Vwk` fields and the `MAP_COL[]` pen→register
> table — NewTOS keeps an equivalent per-device attribute state (ADR-011).

All `vs*` attribute setters share one shape: **read the request → validate
(clamp to a range, or substitute a documented default) → store the accepted
value in the workstation's `Vwk` → echo the *actually selected* value back**
in `INTOUT[0]` (or `PTSOUT` for the pixel-size setters). None can fail — VDI
has no error return; an out-of-range request silently snaps to the default or
the range bound. The dispatcher pre-loads the output counts
(`CONTRL[2]`/`CONTRL[4]`) from the jump table
([`../_dispatcher.md`](../_dispatcher.md)); the setters below do **not**
override them.

---

## Trap interface (all setters below)

Native ABI: GEM trap `svc #2`, selector 115 in `x0`, `VDIPB*` in `x1`
([`../_dispatcher.md`](../_dispatcher.md)). Per function: opcode in
`CONTRL[0]`, workstation handle in `CONTRL[6]` (resolved by the dispatcher;
a bad handle is a silent no-op). Inputs/outputs are per the tables below.

---

## Line attributes (`vdi_line.c`)

| Op | Fn | Reads | Validation → `Vwk` | Echoes |
|----|----|-------|--------------------|--------|
| 15 | `vsl_type`  | `INTIN[0]` style | `MIN_LINE_STYLE..MAX_LINE_STYLE` (1..7) else `DEF_LINE_STYLE` (1); `line_index = v-1` (**0-based**) | `INTOUT[0]` = accepted style (1-based) |
| 16 | `vsl_width` | `PTSIN[0]` width | clamp `[1, SIZ_TAB[6]]`, then **force odd** (even → `w-1`); `line_width = w` | `PTSOUT[0]` = `w`, `PTSOUT[1]` = 0 |
| 17 | `vsl_color` | `INTIN[0]` colour | `validate_color_index`; `line_color = MAP_COL[i]` | `INTOUT[0]` = validated index |
| 108 | `vsl_ends` | `INTIN[0]`=begin, `INTIN[1]`=end | each clamped `MIN_END_STYLE..MAX_END_STYLE` (0..2) else `DEF_END_STYLE` (0=squared); `line_beg`/`line_end` | `INTOUT[0]`/`[1]` = accepted begin/end |
| 113 | `vsl_udsty` | `INTIN[0]` 16-bit mask | `ud_ls = INTIN[0]` (**no validation**) | none |

Source: `vdi_line.c:90` (`vsl_type`), `:104` (`vsl_width`), `:128`
(`vsl_ends`), `:143` (`vsl_color`), `:81` (`vsl_udsty`).

### Value constants (`vdi_defs.h`)

| `#define` | Value | Meaning |
|-----------|-------|---------|
| `MIN_LINE_STYLE` / `MAX_LINE_STYLE` / `DEF_LINE_STYLE` | 1 / 7 / 1 | valid line-type range + default |
| `MAX_LINE_WIDTH` | 40 | ROM cap on line width; the runtime clamp uses `SIZ_TAB[6]` (populated from this) |
| `SQUARED` / `ARROWED` / `ROUND` | 0 / 1 / 2 | line-end styles |
| `MIN_END_STYLE`=`SQUARED` / `MAX_END_STYLE`=`ROUND` / `DEF_END_STYLE`=`SQUARED` | 0 / 2 / 0 | end-style range + default |

### Line types (`INTIN[0]` of `vsl_type`, 1-based; masks in `LINE_STYLE[]`)

`const UWORD LINE_STYLE[6]` (`vdi_line.c:59`) — the six predefined 16-bit
dash masks, indexed by `line_index` (= type−1):

| Type | `line_index` | Mask | Name |
|------|:------------:|:----:|------|
| 1 | 0 | `0xFFFF` | solid |
| 2 | 1 | `0xFFF0` | long dash |
| 3 | 2 | `0xC0C0` | dotted |
| 4 | 3 | `0xFF18` | dash-dot |
| 5 | 4 | `0xFF00` | dashed |
| 6 | 5 | `0xF191` | dash-dot-dot |
| 7 | (user) | `vwk->ud_ls` | user-defined (`vsl_udsty`) — type 7 uses the mask set by op 113 |

### Line-end styles (`vsl_ends`)

| Value | Name |
|-------|------|
| 0 | squared (`SQUARED`) |
| 1 | arrowed (`ARROWED`) |
| 2 | round (`ROUND`) |

Notes:
- **Line widths are always odd** (`vdi_line.c:116-117`) — the wide-line
  algorithm centres on a pixel, so an even request is rounded *down* to the
  next odd value. A request `< 1` clamps to 1; `> SIZ_TAB[6]` clamps to
  `SIZ_TAB[6]`.
- `vsl_udsty` performs **no range check** — any 16-bit value is stored as the
  user line-style mask and takes effect for line type 7.

## Marker attributes (`vdi_marker.c`)

| Op | Fn | Reads | Validation → `Vwk` | Echoes |
|----|----|-------|--------------------|--------|
| 18 | `vsm_type`   | `INTIN[0]` type | `MIN_MARK_STYLE..MAX_MARK_STYLE` (1..6) else `DEF_MARK_STYLE` (3); `mark_index = v-1` (**0-based**) | `INTOUT[0]` = accepted type (1-based) |
| 19 | `vsm_height` | `PTSIN[1]` height | clamp `[DEF_MKHT, MAX_MKHT]`; `mark_height = h`; `mark_scale = (h + DEF_MKHT/2) / DEF_MKHT` (integer) | `PTSOUT[0]` = `mark_scale * DEF_MKWD`, `PTSOUT[1]` = `mark_scale * DEF_MKHT`; also sets `flip_y = 1` (trap `d0` = 1, see edge case 6 below) |
| 20 | `vsm_color`  | `INTIN[0]` colour | `validate_color_index`; `mark_color = MAP_COL[i]` | `INTOUT[0]` = validated index |

Source: `vdi_marker.c:57` (`vsm_type`), `:30` (`vsm_height`), `:72`
(`vsm_color`).

### Value constants (`vdi_defs.h`)

| `#define` | Value | Meaning |
|-----------|-------|---------|
| `MIN_MARK_STYLE` / `MAX_MARK_STYLE` / `DEF_MARK_STYLE` | 1 / 6 / 3 | marker-type range + default (3 = asterisk) |
| `DEF_MKWD`=`SIZ_TAB[8]` / `DEF_MKHT`=`SIZ_TAB[9]` | (device) | base marker cell width / height |
| `MAX_MKWD`=`SIZ_TAB[10]` / `MAX_MKHT`=`SIZ_TAB[11]` | (device) | max marker cell width / height |

### Marker types (`vsm_type`, 1-based; bitmaps `vdi_marker.c:18-23`)

| Type | `mark_index` | Name |
|------|:------------:|------|
| 1 | 0 | dot |
| 2 | 1 | plus |
| 3 | 2 | asterisk (**default**) |
| 4 | 3 | square |
| 5 | 4 | diagonal cross |
| 6 | 5 | diamond |

The device reports the count in `DEV_TAB[8]`; the marker bitmap chosen is
`markhead[mark_index]` (`vdi_marker.c:91-93`).

### Marker height quantisation (`vsm_height`)

`vsm_height` does **not** store the pixel height verbatim. It clamps the
request into `[DEF_MKHT, MAX_MKHT]`, then derives an **integer** scale
`mark_scale = (h + DEF_MKHT/2) / DEF_MKHT` (rounding to the nearest whole
multiple of the base cell). The *actual* marker size returned in `PTSOUT` is
`mark_scale × DEF_MKWD` by `mark_scale × DEF_MKHT` — i.e. the quantised size,
not the request. `mark_scale` multiplies the base marker bitmap at draw time.

---

## Verification criteria

- **`vsl_type` clamp:** `vsl_type(9)` (out of `1..7`) → `INTOUT[0] = 1`
  (`DEF_LINE_STYLE`), `line_index = 0`. `vsl_type(3)` → `INTOUT[0] = 3`,
  `line_index = 2`.
- **`vsl_width` odd + clamp:** on a device with `SIZ_TAB[6] = 40`,
  `vsl_width(8)` → `PTSOUT[0] = 7` (even→odd), `PTSOUT[1] = 0`;
  `vsl_width(0)` → `1`; `vsl_width(100)` → `39` (clamp 40 then force odd).
- **`vsl_ends`:** `vsl_ends(begin=2, end=9)` → `INTOUT = [2, 0]` (2 valid,
  9 out-of-range → `DEF_END_STYLE` 0).
- **`vsl_udsty`:** `vsl_udsty(0x1234)` stores `ud_ls = 0x1234`, no output.
- **`vsm_type` clamp:** `vsm_type(0)` → `INTOUT[0] = 3` (`DEF_MARK_STYLE`),
  `mark_index = 2`; `vsm_type(4)` → `INTOUT[0] = 4`, `mark_index = 3`.
- **`vsm_height` quantise:** with `DEF_MKHT`/`DEF_MKWD` the base cell,
  `vsm_height(PTSIN[1] = DEF_MKHT)` → `mark_scale = 1`,
  `PTSOUT = [DEF_MKWD, DEF_MKHT]`; a height just under `DEF_MKHT` clamps up to
  `DEF_MKHT` (scale 1); a height ≥ `MAX_MKHT` clamps to `MAX_MKHT`.
- **`vs*_color` mapping:** `vsl_color(1)`/`vsm_color(1)` store
  `MAP_COL[1] = numcolors-1` (pen 1 = the highest register); an index
  `< 0` or `≥ numcolors` echoes `1` and stores `MAP_COL[1]`.

---

## Edge cases discovered in source

1. **`vsm_type` round-trip asymmetry (D-VDI-1).** `vsm_type(n)` stores
   `mark_index = n-1`, but `vqm_attributes` (op 36) returns `mark_index`
   **without** the `+1` (`vdi_marker.c:171`) — so a `vsm_type(n)` →
   `vqm_attributes` round-trip reports `n-1`, unlike the line/fill inquiries
   which return their index `+1`. Preserved verbatim; see
   [`../_discrepancies.md`](../_discrepancies.md) D-VDI-1.
2. **Line width is forced odd, always** — `vsl_width(2)` yields **1**, not 2
   (`vdi_line.c:116-117`). Programs that expect their exact even width will
   see one pixel less.
3. **`vsm_height` reads `PTSIN[1]` (the *y* coordinate), not `PTSIN[0]`**
   (`vdi_marker.c:35`) — the height, matching the `(x,y)` pair convention.
4. **`vsl_udsty` has no validation and no output** (`vdi_line.c:81-84`) — the
   raw mask is stored; a mask of 0 makes line type 7 invisible.
5. **The marker `mark_scale` is integer** — fine-grained marker heights are
   quantised to whole multiples of the base cell (`vdi_marker.c:44`).
6. **`vsm_height` sets `flip_y = 1`** (`vdi_marker.c:49`) — the global
   "coordinate vs magnitude" flag consumed by the trap return (`d0`,
   `vdi_asm.S:124-125`; see [`../_dispatcher.md`](../_dispatcher.md)).
   Every other setter in this file leaves `flip_y` at the `0` reset on
   entry to `screen()` (`vdi_main.c:135`); only `vsm_height` (here) and
   `vst_height`/`vst_point`/`vst_font` (text group,
   [`_text.md`](_text.md)) touch it. Not previously documented; see
   [`../_discrepancies.md`](../_discrepancies.md) D-080 (global log).

---

## Dependencies

### Requires
- An open workstation (physical or virtual) with the capability tables
  populated: `SIZ_TAB[6]` (max line width), `SIZ_TAB[8..11]` (marker cell
  sizes), `DEV_TAB[13]` (`numcolors`, via `validate_color_index`).
- `MAP_COL[]` initialised by `init_colors` (`vdi_col.c:708`).

### Used by
- `v_pline`/`v_gdp` (line attrs), `v_pmarker` (marker attrs — which
  temporarily overrides the *line* attributes, `vdi_marker.c:101-114`).
- The inquiry calls `vql_attributes` (35), `vqm_attributes` (36).

---

## Related data structures

- [`../../structures/vwk.md`](../../structures/vwk.md) — the `line_*` /
  `mark_*` / `ud_ls` fields and their defaults.
- [`../_state.md`](../_state.md) — attribute-state defaults table.
- [`../workstation/100-v_opnvwk.md`](../workstation/100-v_opnvwk.md) —
  `SIZ_TAB` byte-index table (marker/line size limits live there).

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the documented
  clamp bounds, the `vsm_type` round-trip value (D-VDI-1), and the
  "line width forced odd" behaviour is **deferred**.

## Configuration / version notes

- These setters are **configuration-independent** — no `CONF_WITH_*` gating.
  The `MAP_COL[]` table size (`MAXCOLOURS`) is 16 unless `EXTENDED_PALETTE`
  (`CONF_WITH_VIDEL || CONF_WITH_TT_SHIFTER`), both **0** in the default
  build (`vdi_col.c:18-22`).

## NewTOS implementation notes (Phase 12)

- Colours are stored as **PEL** (hardware palette index) via `MAP_COL[]`; the
  framebuffer palette resolves PEL → RGB at draw time. `MAP_COL[1]` is
  `numcolors-1` (pen 1 tracks the highest register).
- `SIZ_TAB[6]` (max line width) and the marker cell bounds come from the
  per-device tables (ADR-011), so different screens may clamp differently.
- `validate_color_index` clamps to the device colour count (`DEV_TAB[13]`);
  an out-of-range index maps to **pen 1** (`vdi_control.c:224-230`), *not*
  pen 0. (Corrected file citation — was misattributed to `vdi_col.c`; see
  [`../_discrepancies.md`](../_discrepancies.md) local log D-VDI-15.)
