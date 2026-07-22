# VDI 1:1-Audit — PAKET V2 (line / marker / fill / attributes / contour)

> Auditor, 2026-07-22. Statisch. Ziel-Profil Falcon030/TOS4 make-512. Vom Lead
> persistiert (Auditor-Harness verbietet .md-Schreiben).

**Scope:** `primitives.rs`, `attributes.rs`, `fill_patterns.rs`, `contour.rs`
(v_pline/v_pmarker/v_fillarea/vr_recfl/v_bar + vsl_*/vsm_*/vsf_* + Muster-Tabellen
+ Seed-Fill) gegen `vdi_line.c`/`vdi_marker.c`/`vdi_fill.c` (+ `vdi_defs.h`,
`vdi_control.c`).

## Coverage-Tabelle

| Funktion / Datenobjekt | NewTOS file:line | EmuTOS file:line | Verdikt |
|---|---|---|---|
| `v_pline` (6) | primitives.rs:225-240 | vdi_line.c:1000-1021 | DIVERGENT (arrow/LSTLIN/dash-threading) |
| `polyline`-Kern | primitives.rs:63-81 | vdi_line.c:1101-1121 | DIVERGENT (mask pro Segment reset) |
| `segment`/wide-line | primitives.rs:31-57 | vdi_line.c:1389-1495 | DIVERGENT (keine joins/caps/aspect/arrow) |
| `v_pmarker` (7) | primitives.rs:255-286 | vdi_marker.c:86-162 | MATCH (Glyphen), Rand-Deviation V2-012 |
| Marker-Glyphtabellen | primitives.rs:244-250 | vdi_marker.c:18-23 | MATCH (byte-exakt) |
| `v_fillarea` (9) | primitives.rs:184-220 | vdi_fill.c:617-629 | Struktur-MATCH, Fill DIVERGENT |
| `fill_polygon`/clc_flit | primitives.rs:123-180 | vdi_fill.c:434-610 | DIVERGENT (Top-Zeile off-by-one, x-Rundung) |
| `vr_recfl` (114) | primitives.rs:91-114 | vdi_fill.c:293-314 | DIVERGENT (HOLLOW) |
| `v_bar` (GDP 1) | gdp.rs:64-75 | vdi_gdp.c (bar→draw_rect) | DIVERGENT (HOLLOW) |
| `v_contourfill` (103) | contour.rs:135-245 | vdi_fill.c:948-1091 | Algo abweichend, Region MATCH; HOLLOW divergent |
| `vsl_type/width/color/ends/udsty` | attributes.rs:78-114,241-246 | vdi_line.c:81-150 | MATCH (ends: tote Attr, V2-004) |
| `vsm_type/height/color` | attributes.rs:118-144 | vdi_marker.c:30-79 | MATCH (height: flip_y fehlt, V2-011) |
| `vsf_interior/style/color/perimeter` | attributes.rs:214-306 | vdi_fill.c:173-228 | MATCH |
| `vsf_udpat` (112) | attributes.rs:315-328 | vdi_fill.c:149-166 | DIVERGENT (nur plane 0, V2-009) |
| `vswr_mode` (32) / `vs_clip` (129) | attributes.rs:35-74 | vdi_control.c:235-266 | MATCH |
| `st_fl_ptr`/`select` | fill_patterns.rs:82-106 | vdi_fill.c:338-380 | MATCH-Valid; OOB-Klemme (V2-010) |
| OEMPAT/DITHER/HATCH0/HATCH1 | fill_patterns.rs:21-76 | vdi_fill.c:56-139 | MATCH (byte-exakt) |
| `ROM_UD_PATRN` | — (fehlt) | vdi_fill.c:50-53 | MISSING (V2-008) |
| `LINE_STYLE[6]` | primitives.rs:15 | vdi_line.c:59 | MATCH (byte-exakt) |
| MAX_VERTICES-Cap | primitives.rs:118 / workstation.rs:51 | config.h:389… | DIVERGENT (256 vs 512, V2-005) |

## Findings

### AUD-V2-001: Wide-Line ohne runde Joins/Caps, achsparallele statt senkrechte Verdickung, keine Aspect-Korrektur, keine Pfeile
- Severity: HIGH · Verdikt PLAUSIBLE (Reference, aber beobachtbare Defekte)
- EmuTOS: `wideline()` vdi_line.c:1389-1495 — `cir_dda`(:1240) aspect-korrigierter Viertelkreis, `perp_off`(:1169) senkrechtes 4-Punkt-Parallelogramm je Segment, `do_circ`(:1425/1485) runde Joints/Caps, `arrow`(:1407) Pfeile.
- NewTOS: `segment()` primitives.rs:31-57 — bei width>1 nur achsparallele `-half..=half`-Kopien; kein cir_dda/perp_off/do_circ/arrow (grep-verifiziert).
- Folge: diagonale dicke Linien zu dünn; unausgefüllte Kerben an Gelenken/Enden; ARROWED-Ends ohne Spitze.
- 1:1-Fix: senkrechte Verdickung + do_circ-Joints/Caps + arrow nachbilden. Runtime-verify: yes

### AUD-V2-002: HOLLOW-Interior im Replace-Modus — NewTOS zeichnet nichts, EmuTOS füllt Farbe 0
- Severity: HIGH · Verdikt PLAUSIBLE
- EmuTOS: `draw_rect_common` Replace-Zweig (vdi_line.c:743-782) `pattern=(color&1)?patptr[patind]:0` = 0 → Bereich auf Farbe 0 gelöscht; `st_fl_ptr` HOLLOW patptr=&HOLLOW(0),patmsk=0 (vdi_fill.c:347). Gilt vr_recfl/v_fillarea/v_bar/contourfill.
- NewTOS: HOLLOW → select→None→return, kein Interior-Pixel (primitives.rs:92-95,196; gdp.rs:68; contour.rs:169).
- Folge: `vsf_interior(0)`+Replace über Inhalt: EmuTOS radiert auf 0, NewTOS lässt unverändert.
- **Zusätzlich Spec-Fehler:** `docs/api/vdi/attributes/_fill.md` beschreibt HOLLOW als „nothing drawn" — widerspricht EmuTOS-Quelle (Route: Researcher).
- 1:1-Fix: HOLLOW im Replace-Modus = Fill mit Farbe 0. Runtime-verify: yes

### AUD-V2-003: `fill_polygon` füllt oberste Scanzeile zusätzlich (off-by-one) + andere x-Schnitt-Rundung
- Severity: MEDIUM · Verdikt CONFIRMED (deterministisch)
- EmuTOS: `clc_flit` Schleife `for (y=fill_maxy; y > fill_miny; y--)` (vdi_fill.c:441) überspringt oberste Zeile; x-Schnitt gerundet `((mul_div(dy,2*dx,dytot)+1)>>1)+x` (:480).
- NewTOS: `fill_polygon` `for y in ymin..=ymax` (primitives.rs:142, inklusiv füllt ymin); x-Schnitt truncated ohne Rundung von Vertex 0 (:152).
- Folge: jede gefüllte Fläche/GDP 1px zu hohe Oberkante; Diagonalkanten ±1px. (Am oberen Rand geklippt stimmen beide überein.)
- 1:1-Fix: Schleifengrenze exklusiv oben + mul_div-Rundung. Runtime-verify: yes

### AUD-V2-004: `arrow()` fehlt komplett — `line_beg`/`line_end` (ARROWED/ROUND) tote Attribute
- Severity: MEDIUM · Verdikt CONFIRMED
- EmuTOS: `vdi_v_pline` ruft `arrow()` bei `(line_beg|line_end)&ARROWED` (vdi_line.c:1016,1407,1597).
- NewTOS: line_beg/end nur in vsl_ends gesetzt/echoed (attributes.rs:103), nie von Zeichenlogik konsumiert (grep: kein arrow/do_circ/ARROWED).
- Folge: `vsl_ends(1,1)`+v_pline → EmuTOS Pfeilspitzen, NewTOS nackte Linie. Runtime-verify: yes

### AUD-V2-005: MAX_VERTICES=256 statt make-512-Wert 512
- Severity: MEDIUM · Verdikt CONFIRMED
- EmuTOS: `config.h` MAX_VERTICES=512; clc_flit cappt Crossings bei 512.
- NewTOS: `MAX_VERTS=256` (primitives.rs:118) klemmt Vertices/Crossings; `INQ_TAB[14]=256` (workstation.rs:51). Zusatz: v_pline (primitives.rs:226) klemmt gar nicht.
- Folge: vq_extnd mode1 = 256; v_fillarea >256 Vertices truncated. 1:1-Fix: 512. Runtime-verify: yes

### AUD-V2-006: Dash-Maske nicht über Polyliniensegmente fortgeführt + keine L→R-Normierung + horizontale Linien nicht screen-aligned
- Severity: MEDIUM · Verdikt CONFIRMED (Reference, beobachtbar)
- EmuTOS: `LN_MASK` globaler Zustand, `abline` rotiert per Pixel + schreibt zurück (vdi_line.c:1837,2075) → Dash über alle Segmente; normiert L→R (:2300); horizontale via draw_rect_common screen-aligned (:2330).
- NewTOS: `polyline` gibt jedem Segment feste Maske (primitives.rs:63-81); Backend `draw_line` startet bit=0x8000 je Aufruf, zustandslos (vdi_backend.rs:300).
- Folge: Dash-Phasensprung an jedem Vertex; horizontale Dashes andere Phase. Runtime-verify: yes

### AUD-V2-007: XOR-Modus ohne LSTLIN-Kludge → Doppel-XOR-Löcher an inneren Vertices
- Severity: MEDIUM · Verdikt PLAUSIBLE (Reference, beobachtbar)
- EmuTOS: `abline` WM_XOR && !LSTLIN dekrementiert x2/y2 (vdi_line.c:2319); polyline setzt LSTLIN nur letztes Segment.
- NewTOS: kein LSTLIN; draw_line zeichnet beide Endpunkte inklusiv → gemeinsamer Vertex zweimal ge-XOR-t.
- Folge: `vswr_mode(3)`+v_pline ≥3 Punkte → Pixel-Loch an jedem Knick. Runtime-verify: yes

### AUD-V2-008: `ROM_UD_PATRN` Default-Füllmuster fehlt
- Severity: MEDIUM · Verdikt CONFIRMED
- EmuTOS: init_wk kopiert ROM_UD_PATRN (16 Worte, vdi_fill.c:50-53) in vwk->ud_patrn.
- NewTOS: ROM_UD_PATRN existiert nirgends; `ud_patrn=[0;16]` (vwk.rs:128), init_wk setzt es nicht.
- Folge: `vsf_interior(4)` ohne vsf_udpat → EmuTOS ROM-Muster, NewTOS Null. 1:1-Fix: ROM_UD_PATRN nach fill_patterns.rs + init. (= V1-008.) Runtime-verify: yes

### AUD-V2-009: `vsf_udpat` Multi-Plane auf Plane 0 reduziert (farbige Muster nicht darstellbar)
- Severity: MEDIUM · Verdikt CONFIRMED (Backend-Architektur-Deviation)
- EmuTOS: kopiert count Worte (16 oder INQ_TAB[4]*16) → farbige Muster via multifill.
- NewTOS: `vsf_udpat` speichert immer nur intin(0..16) (attributes.rs:325); multifill gesetzt, aber Pel-Backend ignoriert.
- Folge: Multi-Plane-Muster monochrom. Als Backend-Grenze dokumentieren. Runtime-verify: yes

### AUD-V2-010: `select()` klemmt Hatch/Pattern-Index (D-081-OOB bewusst nicht reproduziert)
- Severity: LOW · Verdikt CONFIRMED (Acceptable Hardening)
- EmuTOS liest bei style-Wechsel ohne vsf_style OOB über HATCH1[96] (D-081). NewTOS klemmt `(fi-6).min(5)`/`(fi-8).min(15)` (fill_patterns.rs:91,99). Bug-für-Bug-OOB-Nachbau wertlos → dokumentieren.

### AUD-V2-011: `flip_y`-Seiteneffekt von `vsm_height` nicht abgebildet (D-080)
- Severity: LOW · Verdikt CONFIRMED (Acceptable ABI-Deviation) — nur über m68k-d0 beobachtbar, native ABI hat es nicht.

### AUD-V2-012: `v_pmarker` erzwingt clip nicht (D-VDI-9 nicht reproduziert)
- Severity: LOW · Verdikt CONFIRMED (Deviation, niedrige Prio)
- EmuTOS erzwingt vwk->clip=1 und stellt nicht zurück (vdi_marker.c:114); NewTOS ändert vwk.clip nicht.

## Zusammenfassung
- **Severity:** HIGH 2 · MEDIUM 6 · LOW 4 (12 Findings / 26 Einheiten).
- **Echte Bugs:** 8 (001-008); **Acceptable/Architektur:** 4 (009-012).
- **Spec-Fehler (Researcher):** V2-002 (`_fill.md` HOLLOW „nothing drawn" widerspricht Quelle).
- **Positiv byte-exakt MATCH:** OEMPAT/DITHER/HATCH0/HATCH1, LINE_STYLE, alle 6 Marker-Glyphen, alle Attribut-Clamp-Ranges.

**Top-3:** V2-001 (Wide-Line joins/caps/arrows), V2-002 (HOLLOW-Replace + Spec-Fehler),
V2-003 (fill_polygon off-by-one). Querbezug: ROM_UD_PATRN (V2-008 = V1-008).
