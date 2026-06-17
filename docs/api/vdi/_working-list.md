# VDI spec extraction — working list (Phase 00b)

> Tracks the per-function VDI spec extraction from EmuTOS @ `1a7484ab`.
> Convention mirrors `docs/api/gemdos/_working-list.md`. The trap /
> calling convention + full opcode map is done — see
> [`_dispatcher.md`](_dispatcher.md). Per-function specs go in
> `docs/api/vdi/<group>/<op>-<name>.md`.
>
> Status: ☐ pending · ◐ drafted · ☑ done. Batches are ordered so the
> Phase-12 implementation can start at the top (lifecycle → attributes
> → output → inquiry → raster/mouse).

## Source map (EmuTOS `vdi/`)

| File | Covers |
|------|--------|
| `vdi_main.c` | dispatch, jump tables, `screen()` entry — **done** (`_dispatcher.md`) |
| `vdi_control.c` | `v_opnwk` / `v_opnvwk` / `v_clswk` / `v_clsvwk` / `v_clrwk`, `vq_extnd`, `vs_clip` |
| `vdi_line.c` | `v_pline`, `vsl_*` (line attributes), `vsl_ends`/`vsl_type`/`vsl_width`/`vsl_color`/`vsl_udsty` |
| `vdi_marker.c` | `v_pmarker`, `vsm_*` |
| `vdi_text.c` / `vdi_textblit.c` / `vdi_tblit.S` | `v_gtext`, `vst_*`, `vqt_*`, font load/unload |
| `vdi_fill.c` | `v_fillarea`, `v_contourfill`, `vr_recfl`, `vsf_*` |
| `vdi_gdp.c` | `v_gdp` (bar/arc/pieslice/circle/ellipse/rounded-rect/justified-text) |
| `vdi_col.c` | `vs_color`, `vq_color`, palette |
| `vdi_raster.c` / `vdi_blit.S` | `vro_cpyfm`, `vrt_cpyfm`, `vr_trnfm`, `v_get_pixel` |
| `vdi_mouse.c` | `v_show_c`/`v_hide_c`, `vsc_form`, `vq_mouse`, `vex_*v`, cursor |
| `vdi_input.c` | `v_locator`/`v_choice`/`v_string`, `vsin_mode`/`vqin_mode`, `vq_key_s` |
| `vdi_esc.c` | `v_escape` sub-functions (cursor/alpha text, inquire) |
| `vdi_misc.c` | `vex_timv`, helpers |
| `vdi_defs.h` | `Vwk`, opcode constants, attribute defaults — **structure spec TODO** |
| `lineavars.S` / `lineaasm.S` | Line-A globals + primitives — see `docs/api/_line-a.md` |

## Batch 1 — workstation lifecycle + clear (D2)

- [x] `v_opnvwk` (100) — [`workstation/100-v_opnvwk.md`](workstation/100-v_opnvwk.md):
      handle alloc + `init_wk` (11 `work_in`) + 45 `INTOUT` (`DEV_TAB`)
      + 12 `PTSOUT` (`SIZ_TAB`). Full work_in/work_out tables.
- [x] `v_opnwk` (1) — [`workstation/01-v_opnwk.md`](workstation/01-v_opnwk.md):
      physical screen ws; `work_in[0] = rez + 2`; shares `init_wk`.
- [x] `v_clsvwk` (101) / `v_clswk` (2) —
      [`workstation/101-v_clsvwk.md`](workstation/101-v_clsvwk.md).
- [x] `v_clrwk` (3) —
      [`workstation/03-v_clrwk.md`](workstation/03-v_clrwk.md). `v_updwk` (4) = no-op.
- [x] `vq_extnd` (102) —
      [`workstation/102-vq_extnd.md`](workstation/102-vq_extnd.md):
      mode 0 = standard tables, mode 1 = `INQ_TAB` (45) + live clip rect.
- [x] **`Vwk` structure spec** →
      [`../structures/vwk.md`](../structures/vwk.md): the attribute area
      grouped (line/marker/fill/text/mode/clip), with the ADR-011
      per-device `device` link noted.

**Batch 1 complete.** Workstation lifecycle (D2) done.

## Batch 2 — attribute setters (D3)

- [x] line + marker — [`attributes/_line-marker.md`](attributes/_line-marker.md):
      `vsl_type` (15), `vsl_width` (16), `vsl_color` (17),
      `vsl_ends` (108), `vsl_udsty` (113); `vsm_type` (18),
      `vsm_height` (19), `vsm_color` (20).
- [x] fill — [`attributes/_fill.md`](attributes/_fill.md):
      `vsf_interior` (23), `vsf_style` (24), `vsf_color` (25),
      `vsf_perimeter` (104), `vsf_udpat` (112).
- [x] text — [`attributes/_text.md`](attributes/_text.md):
      `vst_height` (12), `vst_point` (107), `vst_rotation` (13),
      `vst_font` (21), `vst_color` (22), `vst_effects` (106),
      `vst_alignment` (39).
- [x] mode / clip / colour —
      [`attributes/_mode-clip-color.md`](attributes/_mode-clip-color.md):
      `vswr_mode` (32), `vs_clip` (129), `vs_color` (14).

**Batch 2 complete.** Attribute setters (D3) done. Common shape:
validate (clamp to range, else default) → store in `Vwk` → echo the
accepted value; none can fail.

## Batch 3 — output primitives (D4)

- [ ] `v_pline` (6), `v_pmarker` (7), `v_gtext` (8), `v_fillarea` (9),
      `v_contourfill` (103), `vr_recfl` (114), `v_gdp` (11) +
      sub-functions (bar/arc/pieslice/circle/ellipse/rndrect/justext).

## Batch 4 — inquiry (D5)

- [ ] `vq_color` (26), `vq_extnd` (102, with B1), `v_get_pixel` (105),
      `vql_attributes` (35), `vqm_attributes` (36), `vqf_attributes`
      (37), `vqt_attributes` (38), `vqt_extent` (116), `vqt_width`
      (117), `vqt_name` (130), `vqt_fontinfo` (131), `vqin_mode` (115),
      `vq_key_s` (128).

## Batch 5 — raster + mouse + input (D6)

- [ ] raster: `vro_cpyfm` (109), `vrt_cpyfm` (121), `vr_trnfm` (110).
- [ ] mouse: `v_show_c` (122), `v_hide_c` (123), `vsc_form` (111),
      `vq_mouse` (124), `vex_butv/motv/curv/wheelv` (125–127, 134).
- [ ] input: `v_locator` (28), `v_choice` (30), `v_string` (31),
      `vsin_mode` (33), `vqin_mode` (115).
- [ ] `v_escape` (5) sub-functions (`vdi_esc.c`).
- [ ] timer: `vex_timv` (118).

## Build-config scope (TBD-ADR, matches the Phase-00 profile)

- `CONF_WITH_VDI_16BIT` (Truecolor) — **out** for bring-up (8-bit /
  palette screen first).
- `CONF_WITH_EXTENDED_MOUSE` (wheel) — optional.
- Bezier (`vdi_bezier.c`) — GDP extension, low priority.

## Discrepancy log

Open `docs/api/vdi/_discrepancies.md` when the first source-vs-doc
conflict appears (mirrors the GEMDOS/XBIOS `_discrepancies.md`).
