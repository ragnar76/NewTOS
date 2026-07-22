# VDI 1:1-Audit — MASTER (bottom-up Fix-Backlog)

> Stand: 2026-07-22, Branch `desk-rebuild`. Quelle: 6 Verhaltens-Audits (`v1`–`v6`),
> Funktions-Inventar (`emutos-vdi-function-inventory.md`), Config-Profil
> (`../aes-audit/emutos-config-profile.md`). Gate: `00-GATE.md` (🔴 — AES erst
> nach komplett grünem VDI). Ziel-Profil Falcon030/TOS4 make-512.
> Prinzip Memory `fix-vdi-before-aes-bottom-up`: **alle** Findings, bottom-up.

## 0. REORG — Truecolor-Fundament zuerst (User 2026-07-22)

Die VDI wird auf **Falcon-Truecolor (16-bit RGB565)** umgestellt (Memory
`vdi-truecolor-workstation`, Plan `truecolor-conversion-plan.md`). Das Farbmodell
liegt unter allen Render-Paketen → es kommt **VOR V2/V3/V4**. Neue Reihenfolge:

**TC1–TC5 (Truecolor) → V1-Re-Green → V2 → V3 → V4 → V6.**

TC-Pakete (aus dem 8-Schritt-Plan): TC1 Backend-Modell+per-Vwk-Palette+DEV_TAB/INQ
(1-3, revidiert V1), TC2 Farb-Setter/Query (4), TC3 16-bit-MFDB (5, löst
V5-004/005), TC4 `vr_trnfm16`/`cpyfm16` (6, löst V5-001/002/003/008), TC5
Cursor-Save-under (7, V6-001 akut). Die unten stehenden VW-Pakete gelten
inhaltlich weiter, aber V5 ist in TC aufgegangen und V1 wird in TC1 revidiert.
Status/Tracking: `00-GATE.md`.

## 1. Dashboard

| Dimension | Ergebnis |
|---|---|
| Verhaltens-Findings (V1–V6) | **6 BLOCKER · 8 HIGH · ~24 MEDIUM · ~26 LOW** |
| Funktions-Inventar (269) | HAS 150 · **NO-COUNTERPART 26** · N/A 93 |
| Fundament-Status | V5/Raster 0 BLOCKER/0 HIGH (D-092 geschlossen); V6/Cursor-Unterbau 1:1 |
| Schwerpunkt | **V4/Text: 5 BLOCKER** (Textblit-Kern fehlt); V1: 1 BLOCKER (`v_clswk`) |

**Kernbild:** Das Fundament (Raster/BitBlt V5, Cursor-Semaphor V6) ist 1:1. Die
Masse der echten Bugs sitzt in **Text** (neu geschrieben statt portiert) und in
**Workstation-Tabellen/Primitiven**. Die 26 NO-COUNTERPART sind Feature-Gruppen
(Wideline-Kappen/Pfeile, Font-Skalierung/-Rotation/-Outline, vex-Vektoren).

## 2. Bottom-up Fix-Reihenfolge (Fix-Paket = Audit-Katalog)

Reihenfolge nach Abhängigkeit: **Backend → Workstation → Fonts+Text → Primitive
→ GDP → Mouse.** Jedes Paket: Developer fixt ALLE Findings → unabhängiger
Tester/Auditor bestätigt grün → Lead-Commit → Gate-Zeile 🟩 → nächstes.

### VW1 = V5 — Raster/Backend-Fundament (+ Backend-Capability-Erweiterung)
- Fix: **AUD-V5-001** (vrt_cpyfm PAT_FLAG strippen), **V5-002** (vr_trnfm16
  Truecolor-Zweig), **V5-003** (vr_trnfm chunky-konsistent zu vro_cpyfm),
  V5-004 (Plane-Count-No-Op ohne bb_save zu brechen), V5-005 (vrt Memory-Dest).
- **Capability-Erweiterung (Voraussetzung für VW3/Text + Cursor):**
  `RasterBackend::draw_glyph` um Skalierung/Rotation/Outline-Breite erweitern;
  Cursor-Save-under (V6-001) auf `get_device_pixel`/`put_device_pixel` umstellen.
- Dateien: `vdi/raster.rs`, `vdi/backend.rs`, `kernel/src/vdi_backend.rs`.

### VW2 = V1 — Workstation / Tabellen / Inquiry
- Fix: **AUD-V1-001** (`v_clswk` Opcode 2 als eigene Fn — physische WS bleibt,
  virtuelle schließen; **BLOCKER**), **V1-002** (v_clsvwk PHYS_HANDLE-Guard),
  V1-003 (SIZ_TAB `{5,4,7,13}` via text_init — hängt an VW3-Fonts), V1-004
  (DEV_TAB[3/4]=278), V1-005 (INQ_TAB[14]=512), V1-006 (Handles 16→64), V1-007
  (vq_key_s `&0x0f`), V1-008 (`ROM_UD_PATRN` — auch V2-008), V1-009 (multifill-
  Reset im Dispatcher), V1-010..014 (CUR_WORK/vqt_name/flip_y/vqin_mode/DEV_TAB[5]).
- Dateien: `vdi/workstation.rs`, `vwk.rs`, `mod.rs`, `inquiry.rs`.

### VW3 = V4 — Fonts + Text (braucht VW1-Backend)
- **Fonts/DDA (BLOCKER-Cluster):** AUD-V4-005 (6x6/8x8-Systemfonts + Ring),
  V4-003 (`vst_height` echt, DDA), V4-004 (`vst_point` echt, Verdopplung).
- **Text-Render:** V4-001 (Rotation/chup), V4-002 (Outline), V4-006 (Skew
  SKEWMASK), V4-007 (justified Slack-Verteilung), V4-013 (H-Align Skew/Outline),
  V4-010 ('?'-Ersatz), V4-012 (Underline Light-Maske), V4-011 (Light-Phase).
- Dateien: `vdi/font.rs`, `font_data.rs`, `text.rs`, `attributes.rs`.

### VW4 = V2 — Line / Marker / Fill (braucht VW1-Backend)
- Fix: **AUD-V2-001** (Wide-Line senkrechte Verdickung + do_circ-Kappen/Joints +
  arrow), **V2-002** (HOLLOW-Replace = Farbe 0), **V2-003** (fill_polygon
  Top-Zeile off-by-one + x-Rundung), V2-004 (arrow/ARROWED), V2-005 (MAX_VERTICES
  512 — mit V1-005), V2-006 (Dash-Threading + L→R + screen-align), V2-007 (LSTLIN
  XOR-Kludge), V2-008 (ROM_UD_PATRN — mit V1-008), V2-009 (vsf_udpat multi-plane).
- Dateien: `vdi/primitives.rs`, `attributes.rs`, `fill_patterns.rs`, `contour.rs`.

### VW5 = V3 — GDP / Bezier / Color / Escape (braucht VW4)
- Fix: **AUD-V3-001** (rbox/rfbox Eck-Radius `min(xres>>6,(x2-x1)/2)`, 5 Pkt/Ecke),
  **V3-002** (rbox Linienstil `set_LN_MASK`), V3-003 (justified Interword-Clamp
  nur bei interchar — mit V4-007), V3-004 (v_exit_cur Clear+Hide), V3-005
  (v_curtext Scroll; 0xFF/D-083 = User-Entscheid), V3-006 (arc/ellarc arrows),
  V3-007..012 (justified-Rest, clc_nsteps, Sin/Cos, DAC-Quant, Alpha-Layer,
  enter_cur Reverse).
- Dateien: `vdi/gdp.rs`, `trig.rs`, `req_col.rs`, `escape.rs`.

### VW6 = V6 — Mouse / Input
- Fix: **AUD-V6-001** (Cursor Save-under bit-exakt — in VW1 erledigt, hier
  verifizieren), **V6-002** (vex_butv/motv/curv/wheelv/timv als thin adapter aus
  `feed_relative` aufrufen + Init-Slots), **V6-003** (v_locator GCURX/GCURY +
  dis/hide_cur-Klammer + cur_ms_stat-Terminator), V6-004 (v_string Cap raus +
  SAMPLE-Grenze), V6-005 (vqin_mode stale).
- Dateien: `vdi/mouse.rs`, `input.rs` (+ ggf. `kernel/src/ikbd.rs`-Adapter).

## 3. Die 26 NO-COUNTERPART — Triage (alle in die VW-Pakete eingearbeitet)

| Cluster | # | Fix-Paket |
|---|---|---|
| Wideline-Rundkappen/-gelenke + Pfeilspitzen (cir_dda/do_circ/perp_off/arrow…) | 8 | VW4 (V2-001/004), VW5 (V3-006) |
| Font-Skalierung/-Rotation/-Outline (act_siz/clc_dda/make_header/scale/rotate/outline) | 9 | VW1 (draw_glyph-Capability) + VW3 (V4-001/002/003/004/006) |
| vex-Vektoren nie aufgerufen (mouse_int/mov_cur/tick_int-Bindeglied) | — | VW6 (V6-002) |
| VT52-Alpha-Übergang (esc_init/esc_exit) | 2 | VW5 (V3-004) |
| VDI-Timer-C-Tick-Zustellung (tick_int/timer_init/timer_exit) | 3 | VW6 (V6-002, vex_timv-Adapter) |
| Maus-Locator-Polling + erweiterte Maus + Teardown (gloc_key/vdi_mousex/vdimouse_exit) | 3 | VW6 (V6-003) |
| Pattern-Blit-Modus (setup_pattern) | 1 | VW1 (D-082 — bereits inert korrekt auf vro; vrt = V5-001) |

.S-asm: `vdi_blit.S` überwiegend N/A (16 Ops in `raster_op()` repliziert); `vdi_tblit.S`
= NO-COUNTERPART (rotate/scale/outline → VW1 draw_glyph-Erweiterung); `vdi_asm.S`
= IKBD→Vektor-Bindeglied → VW6.

## 4. Akzeptable Deviations (dokumentiert, NICHT „fixen")
V5-006/007/008 (OOB-Guard, NULL-Guard, MAP_COL-Faltung), V6 (MAP_COL, set_sr
kooperativ, HW-Adapter), V4-016/V1-012 (`flip_y`-d0 unbeobachtbar), V3-009
(Sin/Cos Reference), asm-Blit→RasterBackend, HW-Absence (v_hardcopy). Bleiben als
dokumentierte Abweichung — kein Bug.

## 5. Reihenfolge & Gate
**VW1 (V5) → VW2 (V1) → VW3 (V4) → VW4 (V2) → VW5 (V3) → VW6 (V6).**
Nach jedem Paket: Gate-Zeile in `00-GATE.md` auf 🟩. **Erst wenn alle 🟩 → Gate
öffnet für AES (WP2ff aus `../aes-audit/00-MASTER.md`).** WP1/#127 (AES-Message-
Routing, kein VDI-Bezug) ist bereits committet.
