# 🔴 GATE — VDI muss grün sein, bevor AES angefasst wird

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
| Fundament | **V4 font-data** (font.rs/font_data.rs: 6x6/8x8 + DDA) | ✅ | ❌ | ❌ | 🟡 |
| Fundament | **V5 raster/blit/backend** | ✅ | ❌ | ❌ | 🟡 |
| Attribute | **V1 control/workstation/inquiry** | ✅ | ❌ | ❌ | 🟡 |
| Primitive | **V2 line/marker/fill/attributes/contour** | ✅ | ❌ | ❌ | 🟡 |
| Höher | **V3 gdp/bezier/color/escape** | ✅ | ❌ | ❌ | 🟡 |
| Höher | **V4 text (v_gtext/vst_*/justified)** | ✅ | ❌ | ❌ | 🟡 |
| I/O | **V6 mouse/input** | ✅ | ❌ | ❌ | 🟡 |
| Coverage | **Inventar (269 Fn Presence)** | ⬜ | — | — | ⬜ |

**Gate offen (🔴).** Kein Paket ist grün. Öffnet erst, wenn ALLE Zeilen 🟩 sind.

## Reihenfolge zum Grün-Machen (bottom-up)

Finalisiert im **VDI-Master** (`00-MASTER.md`), dependency-getrieben:

**VW1 (V5 Raster/Backend + draw_glyph-Capability) → VW2 (V1 Workstation) →
VW3 (V4 Fonts+Text) → VW4 (V2 Line/Marker/Fill) → VW5 (V3 GDP/Color/Escape) →
VW6 (V6 Mouse/Input).**

Jedes Paket durchläuft: Developer fixt alle Findings → unabhängiger Tester/Auditor
bestätigt grün → Lead committet → Zeile oben auf 🟩 → nächstes Paket.

**Erst wenn diese Tabelle komplett 🟩 ist, öffnet das Gate für die AES-Fixes.**
