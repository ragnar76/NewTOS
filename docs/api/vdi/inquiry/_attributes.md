# VDI attribute inquiries (Phase 00b D5)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_line.c:156` (`vdi_vql_attributes`),
> `vdi/vdi_marker.c:169` (`vdi_vqm_attributes`), `vdi/vdi_fill.c:321`
> (`vdi_vqf_attributes`), `vdi/vdi_text.c:999` (`vdi_vqt_attributes`).
> Dispatcher jump-table entries + output counts: `vdi/vdi_main.c:67-70`.
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md). Each just
> reads the current [`Vwk`](../../structures/vwk.md) attribute state back
> out — **no device access, no error return**. Colours come back as a
> **VDI pen index** via `REV_MAP_COL[]` (the inverse of the `MAP_COL[]`
> that the setters applied). The attribute-state model these read is
> documented once in [`../_state.md`](../_state.md); this file specifies
> only the query return-slot maps.
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** the opcodes; the `INTOUT`/`PTSOUT` slot→field map; the
> `+1` / 0-based / raw conventions per slot (a query must return exactly
> what the matching setter stored, transformed by the rule below); the
> `flip_y` write where noted. **Reference:** `REV_MAP_COL[]`/`MAP_COL[]`
> storage, the `Vwk` field order — NewTOS keeps an equivalent
> pen↔PEL map and reads its own attribute state.

## Opcodes and output counts (`vdi_main.c:67-70`)

| Op | Fn | `# PTSOUT` (`CONTRL[2]`) | `# INTOUT` (`CONTRL[4]`) | Sets `flip_y`? |
|----|----|:-----------------------:|:------------------------:|:--------------:|
| 35 | `vql_attributes` | 1 | 3 | no |
| 36 | `vqm_attributes` | 1 | 3 | **yes** |
| 37 | `vqf_attributes` | 0 | 5 | no |
| 38 | `vqt_attributes` | 2 | 6 | **yes** |

The dispatcher pre-loads `CONTRL[2]`/`CONTRL[4]` from the jump table
before the handler runs; the handler only fills the slots. (The PTSOUT
count is in **point-pairs**: op 38's "2" = `PTSOUT[0..3]`, 4 WORDs.)

Native ABI (all four): GEM trap `svc #2`, selector 115 in `x0`, `VDIPB*`
in `x1`; opcode in `CONTRL[0]`, handle in `CONTRL[6]`.

---

## `vql_attributes` (35) — line

Source: `vdi_line.c:156-164`.

| Slot | Value | `Vwk` field / setter |
|------|-------|----------------------|
| `INTOUT[0]` | `line_index + 1` (**1-based** style, 1..7) | `vsl_type` (15) stores `type−1` |
| `INTOUT[1]` | `REV_MAP_COL[line_color]` (VDI pen) | `vsl_color` (17) |
| `INTOUT[2]` | `wrt_mode + 1` (1..4) | `vswr_mode` (32) stores `mode−1` |
| `PTSOUT[0]` | `line_width` (device px) | `vsl_width` (16) |
| `PTSOUT[1]` | 0 | — |

Does **not** set `flip_y` (`PTSOUT[0]` is a magnitude but the handler
leaves the flag untouched).

## `vqm_attributes` (36) — marker

Source: `vdi_marker.c:169-179`.

| Slot | Value | `Vwk` field / setter |
|------|-------|----------------------|
| `INTOUT[0]` | `mark_index` (**0-based** — *not* `+1`) | `vsm_type` (18) stores `type−1` |
| `INTOUT[1]` | `REV_MAP_COL[mark_color]` (VDI pen) | `vsm_color` (20) |
| `INTOUT[2]` | `wrt_mode + 1` | `vswr_mode` (32) |
| `PTSOUT[0]` | 0 | — |
| `PTSOUT[1]` | `mark_height` (device px) | `vsm_height` (19) |

Sets `flip_y = 1` (`vdi_marker.c:178`).

> **D-VDI-1** (confirmed): `INTOUT[0]` is returned **un-incremented**,
> while `vql_attributes`/`vqf_attributes` return their style index `+1`.
> `vsm_type(n)` stores `mark_index = n−1`, so a round-trip
> `vsm_type(n)` → `vqm_attributes` reports `n−1`, **not** `n` — an
> internal EmuTOS asymmetry, preserved verbatim (a ported program may
> depend on it). See [`../_discrepancies.md`](../_discrepancies.md)
> (D-VDI-1).

## `vqf_attributes` (37) — fill

Source: `vdi_fill.c:321-331`.

| Slot | Value | `Vwk` field / setter |
|------|-------|----------------------|
| `INTOUT[0]` | `fill_style` (0..4, `FIS_*`) | `vsf_interior` (23) |
| `INTOUT[1]` | `REV_MAP_COL[fill_color]` (VDI pen) | `vsf_color` (25) |
| `INTOUT[2]` | `fill_index + 1` (**1-based** pattern/hatch) | `vsf_style` (24) stores `index−1` (D-VDI-8) |
| `INTOUT[3]` | `wrt_mode + 1` | `vswr_mode` (32) |
| `INTOUT[4]` | `fill_per` (0/1, perimeter flag) | `vsf_perimeter` (104) |

No `PTSOUT`; does **not** set `flip_y`.

> `INTOUT[2] = fill_index+1` **round-trips a value set by `vsf_style`**
> (which stored `index−1`), but **over-reports the open-time default by 1**
> because `init_wk` seeds `fill_index` 1-based while `vsf_style` stores it
> 0-based (D-VDI-8, `vdi_control.c:311` vs `vdi_fill.c:199`). Observable
> only if a program reads `vqf_attributes` immediately after open without
> ever calling `vsf_style`.

## `vqt_attributes` (38) — text

Source: `vdi_text.c:999-1021`.

| Slot | Value | `Vwk` field / setter |
|------|-------|----------------------|
| `INTOUT[0]` | `cur_font->font_id` | `vst_font` (21) |
| `INTOUT[1]` | `REV_MAP_COL[text_color]` (VDI pen) | `vst_color` (22) |
| `INTOUT[2]` | `chup` (rotation, tenths of a degree) | `vst_rotation` (13) |
| `INTOUT[3]` | `h_align` (0..2) | `vst_alignment` (39) |
| `INTOUT[4]` | `v_align` (0..5) | `vst_alignment` (39) |
| `INTOUT[5]` | `wrt_mode + 1` | `vswr_mode` (32) |
| `PTSOUT[0]` | `cur_font->max_char_width` | `vst_font`/`vst_height`/`vst_point` |
| `PTSOUT[1]` | `cur_font->top` (char height) | — |
| `PTSOUT[2]` | `cur_font->max_cell_width` | — |
| `PTSOUT[3]` | `top + bottom + 1` (cell height, scaled-aware) | — |

Sets `flip_y = 1` (`vdi_text.c:1020`). The rotation returned in
`INTOUT[2]` is the value `vst_rotation` stored (0/900/1800/2700/**3600**;
see [`../attributes/_text.md`](../attributes/_text.md)).

---

## Verification criteria

- **`vql_attributes` round-trip:** after `vsl_type(3)`, `vsl_color(2)`,
  `vsl_width(5)`, `vswr_mode(4)` → `INTOUT[0]=3` (1-based),
  `INTOUT[1]=2` (via `REV_MAP_COL`), `INTOUT[2]=4`, `PTSOUT[0]=5`,
  `PTSOUT[1]=0`.
- **`vqm_attributes` 0-based (D-VDI-1):** after `vsm_type(4)` (asterisk
  is default 3) → `INTOUT[0] = 3` (`= 4−1`, un-incremented), **not** 4;
  `flip_y` becomes 1.
- **`vqf_attributes`:** after `vsf_interior(2)` (pattern), `vsf_style(6)`,
  `vsf_perimeter(0)` → `INTOUT[0]=2`, `INTOUT[2]=6` (`= (6−1)+1`),
  `INTOUT[4]=0`. Immediately after open (no `vsf_style`), `INTOUT[2]`
  is one higher than the requested open pattern (D-VDI-8).
- **`vqt_attributes`:** after `vst_font(id)`, `vst_color(1)`,
  `vst_rotation(900)`, `vst_alignment(1,2)`, `vswr_mode(3)` →
  `INTOUT[0]=id`, `INTOUT[2]=900`, `INTOUT[3]=1`, `INTOUT[4]=2`,
  `INTOUT[5]=3`; `PTSOUT[0..3]` = the current font's cell metrics;
  `flip_y` becomes 1.
- **Colour inverse:** `INTOUT[1]` (line/marker/fill/text colour) equals
  the VDI pen index the corresponding setter was given, i.e.
  `REV_MAP_COL[MAP_COL[pen]] == pen`.

## Edge cases discovered in source

1. **`vqm_attributes` returns `mark_index` 0-based** while line/fill
   return `+1` (D-VDI-1, `vdi_marker.c:171`). The only inquiry of the
   four that does not increment its style index.
2. **`vqf_attributes` over-reports the open default by 1** (D-VDI-8): the
   `+1` in `INTOUT[2]` is correct for a `vsf_style`-set value but wrong
   for the `init_wk` open-time default, which is already 1-based.
3. **Only `vqm_attributes` (36) and `vqt_attributes` (38) set `flip_y`**;
   `vql_attributes` (35) and `vqf_attributes` (37) leave it, even though
   35 also returns a magnitude in `PTSOUT[0]`. (`flip_y` matters only for
   an NDC workstation; on the RC screen it is a no-op.)
4. **All four are pure `Vwk` reads** — they never touch the framebuffer
   or the palette; the returned colours are pen indices, not RGB.

## Dependencies

### Requires
- An open workstation whose attribute state has been initialised by
  `init_wk`; `REV_MAP_COL[]` populated at open (inverse of `MAP_COL[]`).

### Used by
- Applications / AES that save-and-restore attribute state around drawing.

## Related data structures

- [`../_state.md`](../_state.md) — the per-`Vwk` attribute-state model
  (fields, defaults, valid ranges) these queries read.
- [`../../structures/vwk.md`](../../structures/vwk.md) — the `Vwk` fields.
- Setters: [`../attributes/_line-marker.md`](../attributes/_line-marker.md),
  [`../attributes/_fill.md`](../attributes/_fill.md),
  [`../attributes/_text.md`](../attributes/_text.md),
  [`../attributes/_mode-clip-color.md`](../attributes/_mode-clip-color.md).

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the
  `vqm_attributes` 0-based return (D-VDI-1) and the `vqf_attributes`
  open-default `+1` (D-VDI-8) against the documented ABI is **deferred**.

## NewTOS implementation notes

- Pure `Vwk` reads — no device access. The only subtlety is the colour
  inverse map: NewTOS stores PEL in the `Vwk`, so `REV_MAP_COL` must
  round-trip exactly with the `MAP_COL` the setters use (per ADR-011,
  per-physical-device).
- Keep the `vqm_attributes` 0-based quirk (D-VDI-1) and the
  `vqf_attributes` `+1` (D-VDI-8) verbatim — they are the de-facto TOS
  ABI.
- `flip_y` is EmuTOS's NDC/RC Y-axis flag; the NewTOS screen workstation
  is RC (Y-down) so PTSOUT is already device coords — carry the flag only
  if/when an NDC workstation is added.
