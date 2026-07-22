# VDI 1:1-Audit — PAKET V4 (text / font)

> Auditor, 2026-07-22. Statisch. Ziel-Profil Falcon030/TOS4 make-512. Vom Lead
> persistiert (Auditor-Harness verbietet .md-Schreiben).

**Scope:** `text.rs`, `attributes.rs` (vst_*), `font.rs`, `font_data.rs` gegen
`vdi_text.c`, `vdi_textblit.c`, `vdi_tblit.S`, `vdi_defs.h`, `bios/font.c`,
`bios/fnt_st_{6x6,8x8,8x16}.c`.

**Kernbefund:** Unser Text-Pfad ist eine handgeschriebene 8x16-Vereinfachung
(`draw_cell`), KEIN 1:1-Port von EmuTOS `text_blt()`. Der Blit-Kern
(`pre_blit`/`rotate`/`scale`/`outline`/`screen_blit`), 2 von 3 Systemfonts und
die DDA-Größenmaschinerie FEHLEN. Die effekt-/größenfreien Setter + die
8x16-Metriken/-Bitmaps sind dagegen 1:1 korrekt.

## Coverage-Tabelle

| Funktion | EmuTOS | NewTOS | Status | Finding |
|---|---|---|---|---|
| `v_gtext` Grundlayout/Align/Farbe | vdi_text.c:546,288 | text.rs:92-114 | teilw. (nur unrot., effektfrei, 8x16) | 001,002,006,010,013 |
| `v_gtext` Rotation (chup) | vdi_text.c:408; textblit.c:442 | text.rs (kein chup) | **FEHLT** | AUD-V4-001 |
| `v_gtext` Outline | textblit.c:266-410 | text.rs:37-57 | **FEHLT** | AUD-V4-002 |
| `v_gtext` Skew | tblit.S:1279 | text.rs:30-32 | erfunden/divergent | AUD-V4-006 |
| `v_gtext` Thicken/Light/Underline | vdi_text.c:327-543 | text.rs:24-87 | mono weitgehend korrekt | 011,012 |
| `v_gtext` char out-of-range | vdi_text.c:464 | font.rs:56 | divergent (kein '?') | AUD-V4-010 |
| `v_justified` (GDP 10) | vdi_text.c:1177 | text.rs:122 | Slot OK, Verteilung divergent | AUD-V4-007 |
| `vst_height` (12) | vdi_text.c:681 | attributes.rs:189 | **STUB** | AUD-V4-003 |
| `vst_point` (107) | vdi_text.c:820 | attributes.rs:195 | **STUB** | AUD-V4-004 |
| `vst_font` (21) | vdi_text.c:939 | attributes.rs:171 | Größen-Redrive fehlt (moot) | AUD-V4-009 |
| `vst_rotation` (13) | vdi_text.c:924 | attributes.rs:159 | **KORREKT 1:1** | — |
| `vst_effects` (106) | vdi_text.c:881 | attributes.rs:148 | Maske 0x1F korrekt | (002) |
| `vst_alignment` (39) | vdi_text.c:887 | attributes.rs:267 | **KORREKT 1:1** | — |
| `vst_color` (22) | vdi_text.c:988 | attributes.rs:257 | MAP_COL-Indirektion fehlt | AUD-V4-014 |
| `vst_load_fonts` (119) | vdi_text.c:1292 | attributes.rs:204 | Return 0 korrekt (kein GDOS) | — |
| `vst_unload_fonts` (120) | vdi_text.c:1353 | attributes.rs:211 | No-op; Reset fehlt (moot) | AUD-V4-015 |
| Textblit-Kern (scale/rotate/outline/pre_blit) | vdi_textblit.c | — | **FEHLT komplett** | 001-004,006 |
| Font-Metriken 8x16 | fnt_st_8x16.c:274 | font.rs:11-52 | **KORREKT 1:1** | — |
| Font-Bitmap 8x16 | fnt_st_8x16.c:14 | font_data.rs:7 | Transposition korrekt (Stichprobe) | — |
| Systemfonts 6x6 + 8x8 | font.c:44; vdi_text.c:604 | — | **FEHLEN** | AUD-V4-005 |
| DDA-Skalierung (clc_dda/make_header) | vdi_text.c:743 | — | **FEHLT** | 003,004 |
| flip_y/d0 (D-080) | vdi_text.c:678 | — | unbeobachtbar → akzeptabel | AUD-V4-016 |

## Findings

### AUD-V4-001: Textrotation nicht gerendert (chup ignoriert) — BLOCKER · CONFIRMED
EmuTOS: `switch(chup)` 0/900/1800/2700 (vdi_text.c:408) + `rotate()` (textblit.c:442,1385). NewTOS: `chup` nie gelesen; `draw_cell` immer horizontal (text.rs, Header gibt es zu). Folge: `vst_rotation(900)`+v_gtext → EmuTOS vertikal, NewTOS waagerecht. 1:1-Fix: 4 Render-Fälle + rotate.

### AUD-V4-002: Outline (F_OUTLINE) akzeptiert, nie gezeichnet — BLOCKER · CONFIRMED
EmuTOS: `outline()` + Breiten-/Positionskorrektur (textblit.c:266-410; vdi_text.c:207,347). NewTOS: `vst_effects` speichert Bit 0x10 (attributes.rs:149), `draw_cell` testet es nie. Folge: umrandete Glyphen fehlen + Center/Right-Anker falsch (V4-013).

### AUD-V4-003: `vst_height` ignoriert Höhe (STUB) — BLOCKER · CONFIRMED
EmuTOS: Ring-Walk + DDA-Skalierung, PTSOUT = tatsächliche Größe (vdi_text.c:681,721). NewTOS: `report_font_size` fix 8x16 {7,13,8,16}, PTSIN[1] ignoriert (attributes.rs:189). No-Stub-Verstoß. Folge: `vst_height(32)` → EmuTOS 2×-hoch, NewTOS normal.

### AUD-V4-004: `vst_point` ignoriert Punktgröße (STUB) — BLOCKER · CONFIRMED
EmuTOS: Font-Wahl nach point, Verdopplung (dda_inc=0xFFFF), INTOUT[0]=echte point (vdi_text.c:820). NewTOS: fix INTOUT[0]=10 (attributes.rs:195). Folge: `vst_point(9)`→EmuTOS 8x8, NewTOS 8x16.

### AUD-V4-005: Systemfonts 6x6 + 8x8 fehlen komplett — BLOCKER · CONFIRMED (MISSING)
EmuTOS: 3 Fontheader font_ring[0]=6x6/[1]=8x8/[2]=8x16, alle font_id=1 (font.c:44; vdi_text.c:604). NewTOS: nur 8x16 (font_data.rs:7); kein Ring/next_font. Root-Ursache von 003/004. Reine Software-Fonts, kein HW-Absence. Folge: jede App mit kleinerer Größe (Editor/TeraDesk/kleine Dialoge) bekommt 8x16 + falsche Metriken.

### AUD-V4-006: Skew (Kursiv) erfunden, divergiert von SKEWMASK — HIGH · CONFIRMED
EmuTOS: `rol.w skew_msk` pro Scanzeile, Zellposition-Shift, Gesamtneigung 8px (tblit.S:1279; fnt_st_8x16.c:284) + calc_width/Align-Korrektur. NewTOS: `shift=(15-row)/5` max 3px, Bit-Shift der Glyphbits (text.rs:30-43). Andere Mechanik/Betrag, keine Breiten-/Align-Korrektur.

### AUD-V4-007: `v_justified` Slack-Verteilung weicht ab — HIGH · CONFIRMED
EmuTOS: delword+Restpixel-Verteilung, Interword-Clamp nur bei interchar (vdi_text.c:1177-1233). NewTOS: word_extra immer ±CELL_W/2 geklemmt, Rest verworfen (text.rs:139-145). Folge: reine Interword-Justierung zu eng, Run trifft max_x nicht. (= V3-003/007, hier vollständiger.)

### AUD-V4-013: H-Align berücksichtigt Skew-/Outline-Breite nicht — MEDIUM · CONFIRMED (bedingt)
EmuTOS: delh = width/2-outline bzw. width-outline*2, width enthält Skew+Outline (vdi_text.c:358-388). NewTOS: width=count*CELL_W (text.rs:99). Für effektfreien Mono-Text = korrekt; mit Skew/Outline daneben.

### AUD-V4-010: Out-of-range-Zeichen kein '?'-Ersatz, sondern Wrap — MEDIUM · CONFIRMED
EmuTOS: `if (temp<first_ade||>last_ade) temp='?'` (vdi_text.c:464). NewTOS: `c & 0xFF` (font.rs:56) → 0x141→'A', -1→0xFF-Glyph.

### AUD-V4-012: Underline ignoriert Light-Maske — MEDIUM · CONFIRMED
EmuTOS: F_UNDER+F_LIGHT → LN_MASK=lighten 0x5555 rotiert (vdi_text.c:513). NewTOS: `underline` immer 0xFFFF (text.rs:84). Folge: graue gepunktete vs. durchgezogene Linie.

### AUD-V4-011: Light-Effekt Schachbrett-Phase am Top statt Bottom verankert — LOW · PLAUSIBLE
EmuTOS: lite_msk rotiert, Blit unten→oben → Phase am Bottom (tblit.S:1249; textblit.c:1206). NewTOS: `effect_row` am Top (text.rs:26) → invertierte Phase, 1px-Versatz.

### AUD-V4-009: `vst_font` treibt Größensetzer nicht nach; flip_y-Leak — LOW · CONFIRMED (moot)
Nur eine Face → Return korrekt; Größen-Neuberechnung + flip_y-Leak fehlen, folgenlos im Single-Font-Zustand. An V4-005 gekoppelt.

### AUD-V4-014: `vst_color` speichert Rohindex statt `MAP_COL[i]` — LOW · PLAUSIBLE (wahrsch. akzeptabel)
EmuTOS: text_color=MAP_COL[r] (HW-PEL) (vdi_text.c:995). NewTOS: Rohindex (attributes.rs:257). Ganzer Farbpfad konsistent Rohindex → Render stimmig, round-trippt gleich; nur bei bit-exaktem PEL-Vergleich beobachtbar. Gehört ins Farb-Subsystem (vdi_col.c/MAP_COL).

### AUD-V4-016: `flip_y`/d0 (D-080) nicht gesetzt — AKZEPTABLES DELTA (kein Bug)
EmuTOS setzt flip_y=1, zurück nur über Trap-d0 (vdi_asm.S:124). NewTOS-Dispatcher gibt `()` → unbeobachtbar über dokumentierte API. Nur relevant, falls je ein NewTOS-Caller VDI-d0 auswertet.

### AUD-V4-015: `vst_unload_fonts` unterlässt State-Reset — INFO (moot)
Kein GDOS/Ring geladen → No-op folgenlos.

## Positiv 1:1 korrekt (kein Finding — der QED-Happy-Path)
- `vst_rotation` (Normalisierung + Rundung bit-genau), `vst_alignment` (H0-2/V0-5), `vst_effects`-Maske 0x1F.
- **8x16-Fonthead-Metriken alle deckungsgleich** (font_id/point/ade/top/ascent/half/descent/bottom/mcw/cell/thicken/ul_size/lighten/skew/name).
- 8x16-Bitmap-Transposition korrekt (`SYSTEM_FONT[N][R]=dat_bytes[R*256+N]`, Stichprobe an Wortgrenzen).
- V-Align-Offsets, Underline-Zeile (top+15), Bold/Thicken für Mono (`b|=b>>1`, keine Breitenerhöhung), Slot-Mapping v_gtext/v_justified.

## Zusammenfassung
- **Severity:** BLOCKER 5 (001-005) · HIGH 2 (006,007) · MEDIUM 3 (010,012,013) · LOW 3 (009,011,014) · INFO/akzeptabel 3 (015,016 + 014 tendenziell).
- **~11 echte Bugs / 13 Fn** (+ komplett fehlender Blit-Kern) — hoch, weil neu geschrieben statt portiert.
- **Runtime-verify offen:** Light-Phase (011), Skew-Pixel (006), Kernel `RasterBackend::draw_glyph` 4 Schreibmodi (außerhalb tos-api).

**Top-3:** V4-001 (Rotation), V4-003/004/005-Cluster (vst_height/point STUB + 6x6/8x8-Fonts + DDA fehlen), V4-002 (Outline). Alle reines Software-VDI, voll über v_gtext/vst_*-PTSOUT beobachtbar.

**OOS:** MAP_COL fehlt in ALLEN *_color-Settern (→ Farb-Subsystem-Audit vdi_col.c); vqt_extent erbt calc_width-Divergenzen bei Skew/Outline (→ Inquiry-Paket).
