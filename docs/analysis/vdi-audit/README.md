# VDI 1:1-Divergenz-Audit — NewTOS-Impl vs EmuTOS-Quelle

> Stand: 2026-07-22, Branch `desk-rebuild`. Analog zur AES-Kampagne
> (`docs/analysis/aes-audit/`). Ziel-Profil: **Falcon030/TOS4, `make 512`**
> (Memory `emutos-target-profile-falcon-tos4`) → jede VDI-Divergenz/Absenz ist
> ein **echter 1:1-Gap**, außer reine HW (Shifter/Videl-Register = RPi4-HAL) oder
> m68k-asm-Blit (durch `RasterBackend` ersetzt). Leitprinzip „match it"
> (Memory `no-rationalizing-emutos-deviations`).

Soll-Referenz: `docs/api/vdi/*.md` (attributes/input/inquiry/output/raster/
workstation + `_dispatcher`/`_escape`/`_state`) und die VDI-Discrepancies im
Haupt-Log `docs/api/_discrepancies.md` (D-079 Dispatcher-No-op, D-080 flip_y,
D-082 PAT_FLAG, D-083 v_escape-12, D-092 vro_cpyfm-Save/Restore, D-095/098).

VDI-ABI: Trap #2, Opcode in `contrl[0]`, fünf Arrays `contrl/intin/ptsin/
intout/ptsout` (NICHT die AES-`int_in/int_out/addr_in`). Auditoren prüfen das
Slot-Mapping gegen `vdi_defs.h` + `docs/api/vdi/_dispatcher.md`.

## Pakete

| Paket | Katalog | Unsere Datei(en) | EmuTOS-Quelle |
|---|---|---|---|
| V1 control/wk/dispatch/inquiry | `v1-control-workstation.md` | `vdi/workstation.rs`, `vwk.rs`, `mod.rs`, `inquiry.rs` | `vdi_control.c`, `vdi_main.c`, `vdi_misc.c` |
| V2 line/marker/fill/attr/contour | `v2-line-marker-fill.md` | `vdi/primitives.rs`, `attributes.rs`, `fill_patterns.rs`, `contour.rs` | `vdi_line.c`, `vdi_marker.c`, `vdi_fill.c` |
| V3 gdp/bezier/color/escape | `v3-gdp-color-escape.md` | `vdi/gdp.rs`, `trig.rs`, `req_col.rs`, `escape.rs` | `vdi_gdp.c`, `vdi_bezier.c`, `vdi_col.c`, `vdi_esc.c` |
| V4 text/font | `v4-text-font.md` | `vdi/text.rs`, `font.rs`, `font_data.rs` | `vdi_text.c`, `vdi_textblit.c` |
| V5 raster/blit/backend | `v5-raster-blit.md` | `vdi/raster.rs`, `backend.rs` | `vdi_raster.c`, `vdi_blit.S`, `vdi_tblit.S` |
| V6 mouse/input | `v6-mouse-input.md` | `vdi/mouse.rs`, `input.rs` | `vdi_mouse.c`, `vdi_input.c` |

Plus `emutos-vdi-function-inventory.md` (269 EmuTOS-VDI-Fn, Presence-Mapping).

## Finding-Format (einheitlich)

Teil 1 — Coverage-Tabelle je EmuTOS-Public-Fn:
`| EmuTOS fn | file:line | NewTOS fn (file:line) | Status (impl/partial/MISSING/STUB) | 1:1? |`

Teil 2 — Divergenz-Findings:
```
### AUD-V<n>-NNN: <fn> — <one-line>
- Severity: BLOCKER | HIGH | MEDIUM | LOW
- EmuTOS:  <file>:<line>
- NewTOS:  <file>:<line> (oder MISSING/STUB)
- Divergenz: <exakt, inkl. contrl/intin/ptsin-Slot-ABI>
- 1:1-Fix:  <was der Code werden muss>
- Runtime-verify: yes/no
```
