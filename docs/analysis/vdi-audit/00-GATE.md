# 🟢 GATE AUFGEHOBEN (2026-07-23) — VDI komplett grün, AES freigegeben

> **Status: OFFEN.** Alle VDI-Pakete (TC1–TC5, V1–V6 + Coverage) sind 1:1
> auditiert, gefixt und je Paket unabhängig grün-getestet (siehe Tabelle unten,
> letzter Commit 796faf1). Die AES-Fixes (WP2ff, `../aes-audit/00-MASTER.md`)
> dürfen ab jetzt beginnen. Der ursprüngliche Blocker-Text bleibt als Historie:

> Harter Blocker (User, 2026-07-22). Memory `fix-vdi-before-aes-bottom-up`.
>
> **BLOCK:** Keine AES-Fix-Arbeit (WP2ff aus `docs/analysis/aes-audit/00-MASTER.md`)
> und nichts, was auf dem VDI aufsetzt, wird begonnen, **solange nicht ALLE
> VDI-Routinen grün auditiert sind.** AES setzt auf VDI-Primitiven auf — auf einem
> kaputten VDI zu fixen erzeugt nur Kompensations-Hacks, die die Wurzelursache
> verdecken.
>
> **„Grün auditiert"** je Paket = (1) auditiert (Katalog geschrieben) **UND**
> (2) alle Findings gefixt (Developer) **UND** (3) re-auditiert 1:1 (unabhängiger
> Auditor/Tester bestätigt: keine offenen echten Findings; nur dokumentierte
> akzeptable Deviations dürfen bleiben — unbeobachtbares `flip_y`-d0,
> m68k-asm-Blit→RasterBackend, HW-Absence wie `v_hardcopy`).
>
> Ausnahme: **WP1/#127** (AES-Message-Routing, kein VDI-Bezug) ist bereits
> committet und vom Gate nicht betroffen.

## Green-Status (bottom-up)

Legende: ⬜ offen · 🟡 auditiert (Findings offen) · 🟩 grün (gefixt + re-auditiert 1:1)

| # | VDI-Paket | Auditiert | Findings gefixt | Re-audit grün | Status |
|---|---|---|---|---|---|
| Fundament | **V4 font-data** (font.rs/font_data.rs: 6x6/8x8 + DDA) | ✅ | ✅ (in V4a, Teil der V4-Kampagne) | ✅ | 🟩 |
| Fundament | **V5 raster/blit/backend** | ✅ | ✅ in TC aufgegangen (TC3b 3d4ab71 + TC4 516dc9a) | ✅ (TC-Tester) | 🟩 |
| Attribute | **V1 control/workstation/inquiry** | ✅ | ✅ (727b33c) + Truecolor-Revision in TC1 (09c9ddc) | ✅ (TC1-Tester) | 🟩 (re-green) |
| Primitive | **V2 line/marker/fill/attributes/contour** | ✅ | V2-line ✅ (637c868) + V2-fill ✅ (406daa0) | ✅ | 🟩 |
| Höher | **V3 gdp/bezier/color/escape** | ✅ | V3-gdp ✅ (70a770b) + V3-escape ✅ (c629ee0) | ✅ | 🟩 |
| Höher | **V4 text (v_gtext/vst_*/justified)** | ✅ | ✅ V4a fonts+DDA + V4b-effects (e896ef3) + V4b-rotate (af2b1f2) + V4-scale (7758e32) + V4c justified (897a249) | ✅ (Tester je Paket) | 🟩 |
| I/O | **V6 mouse/input** | ✅ | V6-001 ✅ (TC5 c7fcff2) + V6-002/003/004/005 ✅ (796faf1) | ✅ (V6-Tester) | 🟩 |
| Coverage | **Inventar (269 Fn Presence)** | ✅ | 26 NO-COUNTERPART in die VW-Pakete triagiert + umgesetzt (MASTER §3) | ✅ | 🟩 |

**🟢 GATE OFFEN (2026-07-23).** ✅ **Alle VDI-Zeilen 🟩 — die komplette VDI ist
1:1 EmuTOS-auditiert, gefixt und je Paket von einem unabhängigen Tester grün
bestätigt.** Der Desktop läuft auf einem echten Falcon-16-bit-RGB565-Truecolor-
Modell (256 Pens, per-Vwk-Palette, 5-bit-Draw, `videl_palette1`, 16-bit-MFDB,
verlustfreies bb_save, bit-exakter Cursor); Text mit vollem Textblit (Rotation/
Skalierung/Effekte/justified 1:1), Primitive/GDP/Escape/Mouse-Input 1:1.

**Dokumentierte akzeptable Deviations** (kein Bug, bleiben): siehe `00-MASTER.md`
§4 (unbeobachtbares `flip_y`-d0, MAP_COL gefaltet, Sin/Cos-Reference, asm-Blit
→ RasterBackend, HW-Absence `v_hardcopy`). **STOP-Split (eigener Task, kein
katalogisiertes Finding):** rotierter `v_justified`-Render (chup≠0 über den
Rotations-Blit), EL0-App-vex-Vektor-Upcall (Signal-Klasse).

**Nur-HW-Restverifikation** (real Pi4, kein QEMU-Pfad): v_locator REQUEST-Blockier-
schleife (Maus+Taste+C: gleichzeitig), v_string >256 Zeichen (IKBD) → gehören in
`docs/api/desktop/_hw-test-checklist.md`.

**→ AES-Fixes freigegeben:** WP2ff aus `../aes-audit/00-MASTER.md` (8 BLOCKER,
15 HIGH). WP1/#127 war bereits committet.

### Truecolor-Fundament (TC, vor allem anderen)

Ersetzt/erweitert das alte VW1 (V5). 8-Schritt-Plan → 5 Fix-Pakete:

| TC | Inhalt (Plan-Schritt) | Fixt | Status |
|---|---|---|---|
| TC1 | Backend `planes()`(=16)+NUM_COLORS→256, DEV_TAB/INQ Truecolor, `ap_planes`-Konsistenz (Schritte 1+3) | V1-Revision (DEV_TAB[13]=256/[39]=0/INQ[4]=16) | 🟩 (09c9ddc) |
| TC2 | per-Vwk-Palette + Farb-Setter/Query Truecolor (`vs/vq_color16` 5-bit, MAP_COL gefaltet) (Schritte 2+4) | V3-010 | 🟩 (abb2280) |
| TC3a | 5-bit-Draw-Pfad über per-Vwk-Palette + `videl_palette1`-Default (der sichtbare Falcon-Look) | Fidelity | 🟩 (2f5cda2) |
| TC3b | Memory-MFDB → 16-bit RGB565 + `bb_save` verlustfrei (Schritt 5) | V5-001/004/005 | 🟩 (3d4ab71) |
| TC4 | `vr_trnfm16` + `vro/vrt_cpyfm16`-Truecolor (6) | V5-002/003/008 | 🟩 (516dc9a) |
| TC5 | Cursor-Save-under bit-exakt (7) | V6-001 (akut) | 🟩 (c7fcff2) |

## Reihenfolge zum Grün-Machen (bottom-up)

Finalisiert im **VDI-Master** (`00-MASTER.md`), dependency-getrieben:

**TC1 → TC2 → TC3 → TC4 → TC5 (Truecolor-Fundament)** → **V1-Re-Green** →
**V2 (Line/Marker/Fill) → V3 (GDP/Color/Escape) → V4 (Fonts+Text) → V6 (Mouse/Input).**
(Truecolor liegt unter dem Farbmodell aller Render-Pakete → zuerst; sonst würden
V2/V3/V4 aufs verworfene 16-Farben-Modell gefixt.)

Jedes Paket durchläuft: Developer fixt alle Findings → unabhängiger Tester/Auditor
bestätigt grün → Lead committet → Zeile oben auf 🟩 → nächstes Paket.

**Erst wenn diese Tabelle komplett 🟩 ist, öffnet das Gate für die AES-Fixes.**
