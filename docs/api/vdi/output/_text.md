# VDI text output `v_gtext` (Phase 00b D4)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_text.c` (`vdi_v_gtext` →
> `output_text`), `vdi/vdi_textblit.c` (glyph blit). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). Attribute state from the
> [`Vwk`](../../structures/vwk.md) (set via the
> [text attribute setters](../attributes/_text.md)). No output, no error
> return. The justified-text variant is GDP sub-op 10 — see
> [`_gdp.md`](_gdp.md).

| Op | Fn | Input | Draws |
|----|----|-------|-------|
| 8 | `v_gtext` | `CONTRL[3]` = char count; `INTIN[]` = char codes (one WORD each); `PTSIN[0..1]` = x,y | the string in the current font/attributes |

## Behaviour (`output_text`)

`vdi_v_gtext(vwk)` calls `output_text(vwk, CONTRL[3], INTIN, -1, NULL)`
(width `-1` = "compute it", no justification). Steps:

1. Quick-out if `count <= 0`.
2. Compute the run width (`calc_width`) for alignment.
3. Snapshot the draw state from the `Vwk` into the blit globals:
   - font: `cur_font` (`FBASE`, `FWIDTH`, monospace flag, scaling
     `DDAINC`/`SCALDIR`/`SCALE` from `vst_height`/`vst_point`).
   - `wrt_mode` → `WRT_MODE`.
   - clip rect (`clip`, `xmn..ymx_clip`).
   - effects `style` → thicken/lighten/skew/underline/outline
     (`WEIGHT`/`LITEMASK`/`SKEWMASK`/`LOFF`/`ROFF`).
   - rotation `chup` → `CHUP` (0/900/1800/2700 on screen).
4. Apply `h_align`/`v_align` to turn `PTSIN` (the alignment point) into
   the glyph origin.
5. Blit each glyph from the font bitmap, advancing per char cell (plus
   any `JUSTINFO` deltas for justified text).

## NewTOS implementation notes

- The bring-up font is the single fixed system bitmap face used by the
  framebuffer text path; `cur_font` always resolves to it until a real
  font subsystem lands, so scaling is limited to integer doubling.
- Char codes arrive **one per `INTIN` WORD** (not packed bytes) — the
  GEMDOS-side ABI passes the array as WORDs.
- Glyph blit honours `wrt_mode` and clips to `vwk->{xmn..ymx}_clip`;
  target framebuffer = `vwk->device` (ADR-011).
- Effects (`style`) and rotation (`chup`) are read here, set by
  [`vst_effects`](../attributes/_text.md) / `vst_rotation`.
