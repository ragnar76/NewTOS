# VDI 1:1-Audit — PAKET V3 (GDP / bezier / color / escape)

> Auditor, 2026-07-22. Statisch. Ziel-Profil Falcon030/TOS4 make-512. Vom Lead
> persistiert (Auditor-Harness verbietet .md-Schreiben).

**Scope:** `v_gdp` (Opcode 11, sub 1-10), Sin/Cos-Tabellen, `v_bez`/`v_bez_fill`,
`vs_color`/`vq_color`, alle `v_escape`-Sub-Opcodes gegen `vdi_gdp.c`,
`vdi_bezier.c`, `vdi_col.c`, `vdi_esc.c`, `vdi_text.c`, `vdi_defs.h`.

## Coverage-Tabelle (Kurz)

| Funktion | EmuTOS | NewTOS | Verdikt |
|---|---|---|---|
| `v_bar` (1) | vdi_gdp.c:434 | gdp.rs:64 | match (hollow-Vorbehalt) |
| `v_arc` (2) | vdi_gdp.c:210 | gdp.rs:80 | Div (V3-006/008/009) |
| `v_pieslice` (3) | vdi_gdp.c:199 | gdp.rs:80 | match |
| `v_circle` (4) | vdi_gdp.c:378 | gdp.rs:85 | Div (008/009; yrad-Aspekt OOS) |
| `v_ellipse` (5) | vdi_gdp.c:381 | gdp.rs:85 | match |
| `v_ellarc` (6) | vdi_gdp.c:210 | gdp.rs:80 | Div (006/008/009) |
| `v_ellpie` (7) | vdi_gdp.c:199 | gdp.rs:80 | match |
| `v_rbox` (8) | vdi_gdp.c:229 | gdp.rs:152 | **BUG** (V3-001 Radius, V3-002 Linienstil) |
| `v_rfbox` (9) | vdi_gdp.c:334 | gdp.rs:152 | **BUG** (V3-001 Radius) |
| `v_justified` (10) | vdi_text.c:1177 | text.rs:122 | **BUG** (V3-003 Clamp, V3-007 Rest) |
| `clc_nsteps` | vdi_gdp.c:347 | gdp.rs:110 | Div (V3-008) |
| Sin/Cos | vdi_gdp.c:57 | trig.rs:12 | Div (V3-009 Bhaskara vs Tabelle) |
| `v_bez`/`v_bez_fill` | vdi_bezier.c (HAVE_BEZIER 0) | gdp.rs:35 `_=>{}` | match (no-op=no-op) |
| `vs_color` (14) | vdi_col.c:656 | attributes.rs:226 | match |
| `vq_color` (26) flag0 | vdi_col.c:875 | inquiry.rs:90 | match |
| `vq_color` (26) flag≠0 | vdi_col.c:897 | inquiry.rs:93 | Div (V3-010 keine DAC-Quant.) |
| `v_escape` 1 vq_chcells | vdi_esc.c:62 | escape.rs:63 | match |
| esc 2 v_exit_cur | vdi_esc.c:81 | escape.rs:69 | **BUG** (V3-004) |
| esc 3 v_enter_cur | vdi_esc.c:93 | escape.rs:70 | Div (V3-012) |
| esc 4-11 cursor/eeos/eeol/addr | vdi_esc.c:102-183 | escape.rs:77-102 | match |
| esc 12 v_curtext | vdi_esc.c:196 | escape.rs:103 | **BUG** (V3-005; 0xFF=D-083) |
| esc 13-15 rvon/rvoff/qcuraddr | vdi_esc.c:213-236 | escape.rs:119-126 | Div (V3-011 privater Layer) |
| esc 16-19 + >19 | vdi_esc.c:248-349 | escape.rs:127-141 | match (17 hardcopy=HW-absence) |

## Findings

### AUD-V3-001: `v_rbox`/`v_rfbox` Eck-Radius-Formel weicht komplett ab
- Severity: HIGH · CONFIRMED
- EmuTOS: `xradius=min(xres>>6,(x2-x1)/2)`, `yradius=min(mul_div(xradius,xsize,ysize),(y1-y2)/2)`, 5 Punkte/Ecke fix 0/22.5/45/67.5/90° (vdi_gdp.c:254-271).
- NewTOS: `r=(w.min(h)/8).clamp(1,16)`, 9 Punkte/Ecke lineare 90/8-Schritte (gdp.rs:157-180). Ecken-Reihenfolge TR→TL→BL→BR (EmuTOS UR→LR→LL→UL).
- Folge: sichtbar andere Eckenrundung (screengrößen- vs. boxgrößenabhängig). Spec `_gdp.md:121-125` beschreibt EmuTOS korrekt, Impl folgt nicht.
- 1:1-Fix: `xradius=min(xres>>6,(x2-x1)/2)`, yradius analog, 5 Punkte/Ecke.

### AUD-V3-002: `v_rbox` ignoriert Linienstil (Maske hartkodiert 0xFFFF)
- Severity: HIGH · CONFIRMED
- EmuTOS: `set_LN_MASK(vwk)` vor polyline (vdi_gdp.c:324); LN_MASK = LINE_STYLE[l] bzw. ud_ls (vdi_line.c:2389).
- NewTOS: `gdp.rs:191` polyline mit `0xFFFF` (solid) hartkodiert. Inkonsistent zur eigenen `curve()` (gdp.rs:144 nutzt line_mask korrekt).
- Folge: `vsl_type(dashed)`+v_rbox → EmuTOS gestrichelt, NewTOS solid. 1:1-Fix: `line_mask(vwk)` statt `0xFFFF`.

### AUD-V3-003: `v_justified` Interword-Clamp unbedingt statt nur bei interchar
- Severity: MEDIUM · CONFIRMED
- EmuTOS: Clamp ±max_cell_width/2 nur `if (interchar)` (vdi_text.c:1222); reines Interword verteilt vollen Slack.
- NewTOS: `text.rs:139-143` immer geklemmt auf ±4.
- Folge: "A B" interword max_x=100: EmuTOS Run≈100, NewTOS Run≈28 (massiv unter-justiert). 1:1-Fix: Clamp nur bei interchar.

### AUD-V3-004: `v_escape` sub 2 `v_exit_cur` no-op statt Clear+Cursor-Hide
- Severity: MEDIUM · PLAUSIBLE
- EmuTOS: `bios_conws("\033H\033f\033E")` = home, Cursor-Hide, Clear-and-home (vdi_esc.c:81).
- NewTOS: `escape.rs:69` `2 => {}`. Folge: `v_escape(2)` löscht Textkonsole nicht. 1:1-Fix: Clear+home+hide.

### AUD-V3-005: `v_escape` sub 12 `v_curtext` kein Scroll am unteren Rand
- Severity: MEDIUM · CONFIRMED
- EmuTOS: `crawio(*chr++)` → VT-52-Konsole scrollt am Bildende (vdi_esc.c:196).
- NewTOS: `escape.rs:111-113` y bei my geklemmt, x wrappt → überschreibt letzte Zeile, kein Scroll. Zusätzlich: 0xFF wird von uns gezeichnet (escape.rs:109), EmuTOS gibt es wg. Crawio-Sentinel NICHT aus (D-083) → NewTOS „korrekter", divergiert (User-Entscheidung Bug-replizieren).

### AUD-V3-006: `v_arc`/`v_ellarc` keine Pfeilspitzen (arrow)
- Severity: MEDIUM-LOW · CONFIRMED
- EmuTOS: `if ((line_beg|line_end)&ARROWED) arrow(...)` (vdi_gdp.c:215). NewTOS: kein arrow projektweit (line_beg/end nur gespeichert). = globale Arrow-Auslassung (auch v_pline, V2-004).

### AUD-V3-007: `v_justified` Divisions-Rest (Slack) verworfen
- Severity: LOW · CONFIRMED
- EmuTOS verteilt rmword/rmchar pixelweise → Run trifft max_x exakt (vdi_text.c:1214,479). NewTOS: Integer-Div, Rest weg → bis zu (Lücken−1)px zu kurz.

### AUD-V3-008: Bogen-Segmentzahl radius- vs. sweep-basiert
- Severity: LOW · CONFIRMED (Reference)
- EmuTOS: `steps=max(xrad,yrad)>>2` clamp[32,128] (vdi_gdp.c:347). NewTOS: `(sweep/60).max(16).min(128)` (gdp.rs:112), Untergrenze 16 statt 32.

### AUD-V3-009: Sin/Cos Bhaskara-Approx statt 113-Einträge-Tabelle
- Severity: LOW · PLAUSIBLE (Reference)
- EmuTOS: 113-Einträge sin_tbl + lineare Interpolation + Achsen-Snap (vdi_gdp.c:57). NewTOS: Bhaskara-I Q15/32767, kein Snap (trig.rs). Fehler ~0.16% → gelegentl. 1px. Contract stellt Sinustabelle als Reference frei; gegen „exakt"-Auftrag geflaggt.

### AUD-V3-010: `vq_color` flag≠0 ohne DAC-Quantisierung
- Severity: LOW-MED · PLAUSIBLE
- EmuTOS: liest echten HW-Wert zurück, DAC-quantisiert (Videl 500→502) (vdi_col.c:897). NewTOS: `backend.query_color` gibt last-requested unquantisiert (backend.rs:97). Klein fürs Falcon-Ziel, aber Contract. Quant. liegt im Backend (teilw. OOS).

### AUD-V3-011: Alpha-Layer privat statt geteilte BIOS-VT-52-Konsole
- Severity: LOW-MED · PLAUSIBLE
- EmuTOS: alle Alpha-Escapes treiben die eine BIOS-Konsole (geteilter Cursor/Reverse). NewTOS: eigene CUR_X/CUR_Y/REVERSE-Statics auf RasterBackend (escape.rs:19), entkoppelt. Bewusste Architektur (escape.rs:5-8); bei gemischter Konsole+Alpha inkonsistent.

### AUD-V3-012: `v_enter_cur` setzt Reverse-Video zurück; EmuTOS nicht
- Severity: LOW · CONFIRMED
- EmuTOS: `"\033E\033e\015"` (Clear+home+Cursor-show+CR), kein ESC q → Reverse bleibt (vdi_esc.c:93). NewTOS: `escape.rs:75` REVERSE=false, kein Cursor.

## Zusammenfassung
- **Severity:** HIGH 2 · MED 3 (Bugs) · LOW/MED-LOW 7 (Divergenzen) = 12 Findings.
- **Echte BUGs (Contract):** 5 (001-005); **Divergenzen:** 7 (006-012).
- **CONFIRMED:** 8 · **PLAUSIBLE:** 4.
- **Positiv korrekt:** Slot-Mapping (contrl[5]/ptsin-Radien/intin), Winkel-Wrap, geschlossene-Form-Perimeter, vs_color raw+clamp, vq_color flag0, Escape-Dispatch 0-19+99, Bezier no-op (HAVE_BEZIER 0).

**Top-3:** V3-001 (rbox Radius), V3-002 (rbox Linienstil), V3-003 (justified Clamp).

**OOS-Notizen:** globale Arrow-Auslassung (auch v_pline); yrad-Aspekt ausgelassen (quadr. Pixel → unbeobachtbar); MAP_COL Identity statt EmuTOS-nicht-identisch (→ v_get_pixel PEL, Palette-Modell); 16-bit-Farbpfad nicht impl (num_colors=16 → unerreichbar, akzeptabel).
