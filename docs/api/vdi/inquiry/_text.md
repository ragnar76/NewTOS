# VDI text-metric inquiries (Phase 00b D5)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_text.c:1024` (`vdi_vqt_extent`),
> `:1062` (`vdi_vqt_width`), `:1100` (`vdi_vqt_name`), `:1137`
> (`vdi_vqt_fontinfo`); helpers `calc_width` (`:176`), `calc_height`
> (`:160`), `act_siz` (scaling). Dispatcher entries + output counts:
> `vdi/vdi_main.c:91,92,105,106`. Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). All read the current
> [`Vwk`](../../structures/vwk.md) font/effects/scaling state; no error
> return except `vqt_width`'s out-of-range `−1` marker.
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** the opcodes; the input slots; the return-slot maps
> (esp. the `vqt_extent` 4-corner order per rotation, the `vqt_fontinfo`
> 10-slot `PTSOUT`, the `vqt_width` `−1` marker); the `flip_y` write. **Reference:**
> `calc_width`/`calc_height`, `off_table`/`hor_table`, the DDA scaling,
> `font_ring` — NewTOS reimplements the metric engine with its own font
> subsystem, but must reproduce the returned numbers.

## Opcodes and output counts (`vdi_main.c`)

| Op | Fn | Input | `# PTSOUT` | `# INTOUT` | Sets `flip_y`? |
|----|----|-------|:----------:|:----------:|:--------------:|
| 116 | `vqt_extent`   | `CONTRL[3]`=char count, `INTIN[]`=codes | 4 | 0 | **yes** |
| 117 | `vqt_width`    | `INTIN[0]`=char code (ADE) | 3 | 1 | **yes** |
| 130 | `vqt_name`     | `INTIN[0]`=font element (1-based) | 0 | 33 | no |
| 131 | `vqt_fontinfo` | — | 5 | 2 | **yes** |

(`PTSOUT` count is in point-pairs: op 116's "4" = `PTSOUT[0..7]`,
8 WORDs; op 131's "5" = `PTSOUT[0..9]`, 10 WORDs.) Native ABI (all): GEM
trap `svc #2`, selector 115 in `x0`, `VDIPB*` in `x1`; opcode in
`CONTRL[0]`, handle in `CONTRL[6]`.

---

## `vqt_extent` (116) — string bounding box

Source: `vdi_text.c:1024-1059`. Computes `width = calc_width(vwk,
CONTRL[3], INTIN)` and `height = calc_height(vwk)`, zeroes
`PTSOUT[0..7]`, then writes the **four corner points** of the
(rotated) bounding rectangle per `chup`:

| `chup` | P1 `(PTSOUT[0,1])` | P2 `(PTSOUT[2,3])` | P3 `(PTSOUT[4,5])` | P4 `(PTSOUT[6,7])` |
|:------:|--------------------|--------------------|--------------------|--------------------|
| 0 / 3600 | `(0, 0)` | `(width, 0)` | `(width, height)` | `(0, height)` |
| 900  | `(height, 0)` | `(height, width)` | `(0, width)` | `(0, 0)` |
| 1800 | `(width, height)` | `(0, height)` | `(0, 0)` | `(width, 0)` |
| 2700 | `(0, width)` | `(0, 0)` | `(height, 0)` | `(height, width)` |

(The rows read out the `PTSOUT[k]` assignments in `vdi_text.c:1033-1056`;
slots not assigned stay 0 from the `bzero`.) Sets `flip_y = 1`
(`:1058`).

- `width` accumulates each char's `off_table[chr+1] − off_table[chr]`
  (or `cnt × cell` for a monospace font), then applies scaling
  (`dda_inc == 0xFFFF` ⇒ ×2, else `act_siz`) and the style-effect
  additions — thicken adds `cnt × thicken`, skew adds
  `left_offset + right_offset`, outline adds `cnt × 2`
  (`calc_width`, `vdi_text.c:176-210`).
- `height = top + bottom + 1` (+2 if `F_OUTLINE`) (`calc_height`,
  `:160-170`).

## `vqt_width` (117) — one character's cell width + offsets

Source: `vdi_text.c:1062-1096`. For the ADE code `k = INTIN[0]`:

| Slot | Value |
|------|-------|
| `INTOUT[0]` | `k` (echoes the char), or **`−1`** if `k < first_ade` or `k > last_ade` |
| `PTSOUT[0]` | cell width = `off_table[k'+1] − off_table[k']` (`k' = k − first_ade`), scaled by `dda_inc`/`act_siz` if `scaled` |
| `PTSOUT[2]` | left horizontal offset = `hor_table[k'*2]` if `F_HORZ_OFF`, else `0` |
| `PTSOUT[4]` | right horizontal offset = `hor_table[k'*2+1]` if `F_HORZ_OFF`, else `0` |

`PTSOUT[2]`/`PTSOUT[4]` are pre-zeroed (`:1072-1073`); the offsets are
non-zero only for a font whose header has the `F_HORZ_OFF` flag. On the
`−1` (no-such-char) path `PTSOUT[0]` is left unset (0 default). Sets
`flip_y = 1` (`:1095`).

## `vqt_name` (130) — font-element name

Source: `vdi_text.c:1100-1134`. Walks `font_ring` counting **distinct
`font_id`s**; the `INTIN[0]`-th (**1-based**) distinct face is selected.
Out of range → the built-in system font `fon6x6` (`:1125-1126`).

| Slot | Value |
|------|-------|
| `INTOUT[0]` | the selected face's `font_id` |
| `INTOUT[1..]` | face name, **one character per WORD**, NUL-terminated then zero-padded to `FONT_NAME_LEN` (32) |

So `INTOUT[0]` + 32 name WORDs = 33 output WORDs (matches the jump-table
`nintout = 33`). Does **not** change `cur_font`; does not set `flip_y`.

## `vqt_fontinfo` (131) — current-font metrics

Source: `vdi_text.c:1137-1174`. Reports the **current** font's metrics,
adjusted for the active `style` bits:

| Slot | Value |
|------|-------|
| `INTOUT[0]` | `first_ade` (first character code) |
| `INTOUT[1]` | `last_ade` (last character code) |
| `PTSOUT[0]` | `max_cell_width` |
| `PTSOUT[1]` | `bottom` (below baseline) |
| `PTSOUT[2]` | `thicken` if `F_THICKEN` active, else `0` |
| `PTSOUT[3]` | `descent` |
| `PTSOUT[4]` | `left_offset` if `F_SKEW` active, else `0` |
| `PTSOUT[5]` | `half` (half-line height) |
| `PTSOUT[6]` | `right_offset` if `F_SKEW` active, else `0` |
| `PTSOUT[7]` | `ascent` |
| `PTSOUT[8]` | `0` (reserved) |
| `PTSOUT[9]` | `top` (above baseline) |

The thicken (`PTSOUT[2]`) and skew offsets (`PTSOUT[4]`/`[6]`) are
returned as their font-header values **only when the corresponding effect
is currently enabled** in `vwk->style`; otherwise `0`. Sets `flip_y = 1`
(`:1173`).

---

## Verification criteria

- **`vqt_extent` unrotated:** for a `w×h` string with `chup=0` →
  `PTSOUT = [0,0, w,0, w,h, 0,h]`; `flip_y` becomes 1.
- **`vqt_extent` 90°:** same string with `chup=900` →
  `PTSOUT = [h,0, h,w, 0,w, 0,0]` (width and height swap axes per the
  table).
- **`vqt_width` valid:** for a char in `[first_ade,last_ade]` →
  `INTOUT[0]=char`, `PTSOUT[0]` = that glyph's advance width; offsets 0
  for a font without `F_HORZ_OFF`.
- **`vqt_width` out of range:** `vqt_width(first_ade − 1)` →
  `INTOUT[0] = -1`.
- **`vqt_name` in range:** `vqt_name(1)` → `INTOUT[0]` = first distinct
  face's id, `INTOUT[1..]` = its name one-char-per-WORD, zero-padded to
  32; `cur_font` unchanged.
- **`vqt_name` out of range:** `vqt_name(9999)` → the system font
  `fon6x6`'s id + name.
- **`vqt_fontinfo` skew slots:** with `F_SKEW` set in `style`,
  `PTSOUT[4]`/`PTSOUT[6]` = the font's `left_offset`/`right_offset`;
  cleared to 0 when skew is off.

## Edge cases discovered in source

1. **`vqt_extent` writes 4 corners, not a `w×h` pair** — the box is
   traced as a polygon whose corner order rotates with `chup`
   (`vdi_text.c:1032-1057`). A program expecting `PTSOUT[0..1] = w,h`
   reads wrong data; corner P1 is the rotated origin.
2. **`vqt_width` returns `−1` for an out-of-range char** in `INTOUT[0]`
   and leaves `PTSOUT[0]` at 0 (`:1076-1077`).
3. **`vqt_name` is 1-based and never fails** — element 0 or an
   over-large index defaults to the system font, still returning a valid
   id + name (`:1108,1125-1126`).
4. **`vqt_name` does not change `cur_font`** — unlike `vst_font`, it only
   reports; the current face is untouched.
5. **`vqt_fontinfo` thicken/skew slots are style-gated** — the offsets
   are 0 unless the effect is *currently* active, so the same font
   reports different metrics as `vst_effects` toggles bold/italic
   (`vdi_text.c:1152-1167`).
6. **`vqt_extent`/`vqt_width` honour scaling** — a scaled font (`scaled`,
   `dda_inc`) doubles or `act_siz`-scales the widths (`calc_width`
   `:194-199`, `vqt_width` `:1082-1087`), so the metrics track the
   `vst_height`/`vst_point` state.

## Dependencies

### Requires
- An open workstation with `cur_font` set and `font_ring` populated by
  `text_init`/`text_init2`; the scaling state (`scaled`/`dda_inc`) and
  `style` from the text setters.

### Used by
- Applications laying out text; AES text rendering; the justified-text
  GDP (`gdp_justified`, [`../output/_gdp.md`](../output/_gdp.md)) uses
  the same `calc_width` engine.

## Related data structures

- [`../../structures/vwk.md`](../../structures/vwk.md) — `cur_font`,
  `font_ring`, `style`, `scaled`/`dda_inc`.
- [`../attributes/_text.md`](../attributes/_text.md) — the setters
  (`vst_font`/`vst_height`/`vst_point`/`vst_effects`/`vst_rotation`)
  whose state these queries report.
- `Fonthead` (`include/fonthdr.h`) — `first_ade`/`last_ade`/`top`/
  `bottom`/`ascent`/`descent`/`half`/`thicken`/`left_offset`/
  `right_offset`/`off_table`/`hor_table`; `FONT_NAME_LEN = 32`.

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the
  `vqt_extent` per-rotation corner order and the `vqt_fontinfo`
  `PTSOUT` slot meanings against the documented ABI is **deferred**.

## NewTOS implementation notes

- With the single bring-up bitmap font, `vqt_name` reports one face and
  `vqt_extent`/`vqt_width` use that font's fixed cell metrics (×2 when
  integer-doubled). The `font_ring`/`off_table`/`hor_table` machinery
  stays in the spec for when a real font subsystem lands.
- These are read-only metric queries; no framebuffer access.
- Rotation handling in `vqt_extent` mirrors
  [`vst_rotation`](../attributes/_text.md) — the same 90° snapping; keep
  the 4-corner order per `chup` as contract.
