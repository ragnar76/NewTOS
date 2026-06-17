# VDI text attribute setters (Phase 00b D3)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_text.c`. Setter shape as in
> [`_line-marker.md`](_line-marker.md). The height/font setters walk
> the loaded-font chain to pick the **nearest available** face/size and
> report the *actual* metrics. No error return.

| Op | Fn | Input | Effect → `Vwk` | Output |
|----|----|-------|----------------|--------|
| 22 | `vst_color`     | `INTIN[0]` colour | `validate_color_index`; `text_color = MAP_COL[i]` | `INTOUT[0]` = validated index |
| 106 | `vst_effects`  | `INTIN[0]` style bits | `style = INTIN[0] & INQ_TAB[2]` (mask to supported = `0x1F`) | `INTOUT[0]` = accepted bits |
| 39 | `vst_alignment` | `INTIN[0]`=horiz, `INTIN[1]`=vert | clamp horiz `0..2`; vert similarly; `h_align`/`v_align` | `INTOUT[0]`/`[1]` = accepted |
| 13 | `vst_rotation`  | `INTIN[0]` angle (1/10°) | normalise to `0..3599`; on screen snapped to 0/900/1800/2700; sets `chup` | `INTOUT[0]` = actual angle |
| 21 | `vst_font`      | `INTIN[0]` face id | walk loaded fonts → nearest matching face at the current size; `cur_font` | `INTOUT[0]` = selected face id |
| 12 | `vst_height`    | `PTSIN[0]`=width, `PTSIN[1]`=height (device coords) | find the font height nearest the request; set `cur_font` + scaling (`dda_inc`, `scaled`, `t_sclsts`) | `PTSOUT[0..3]` = actual char width, char height, cell width, cell height |
| 107 | `vst_point`    | `INTIN[0]` point size | like `vst_height` but in **points**; `pts_mode = TRUE` | `INTOUT[0]` = actual point size; `PTSOUT[0..3]` = actual sizes |

### Text effects bits (`vst_effects`, masked by `0x1F`)

| Bit | Effect |
|-----|--------|
| 0x01 | thickened (bold) |
| 0x02 | light (greyed) |
| 0x04 | skewed (italic) |
| 0x08 | underlined |
| 0x10 | outlined |

### Alignment (`vst_alignment`)

Horizontal `0..2` = left / centre / right. Vertical = baseline /
half / ascent / bottom / descent / top (TOS uses 0..5; EmuTOS clamps).

## NewTOS implementation notes

- The bring-up font is a single fixed bitmap face (the framebuffer
  text path's system font); `vst_font`/`vst_load_fonts` therefore
  always resolve to that face until a real font subsystem lands. So
  `vst_height`/`vst_point` report the one available cell size (and any
  integer doubling we support), not arbitrary scaling.
- On a raster screen, text rotation is limited to the four right
  angles (`chup` ∈ {0,900,1800,2700}); arbitrary angles snap — matches
  EmuTOS's screen driver.
- `style`/`h_align`/`v_align`/`chup` are consumed by `v_gtext`
  (opcode 8) at draw time.
