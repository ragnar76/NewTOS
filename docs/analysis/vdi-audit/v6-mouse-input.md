# VDI 1:1-Audit — PAKET V6 (mouse / input)

> Auditor, 2026-07-22. Statisch. Ziel-Profil Falcon030/TOS4, WITH_AES. Vom Lead
> persistiert (Auditor-Harness verbietet .md-Schreiben).

**Scope:** `mouse.rs` + `input.rs` gegen `vdi_mouse.c`, `vdi_input.c`
(+ vdi_main.c-Dispatcher, vdi_misc.c vex_timv, bios.c tickcal).

## Coverage-Tabelle

| EmuTOS fn | file:line | NewTOS fn (file:line) | Status | 1:1? |
|---|---|---|---|---|
| `vsc_form`/set_mouse_form | vdi_mouse.c:513,450 | vsc_form (mouse.rs:272) | impl | ja* (MAP_COL weggelassen, PEL-Backend) |
| `v_show_c`/linea_show_mouse | vdi_mouse.c:321,1125 | v_show_c (mouse.rs:315) | impl | **ja 1:1** |
| `v_hide_c`/hide_cur | vdi_mouse.c:331,157 | v_hide_c (mouse.rs:337) | impl | **ja 1:1** |
| `dis_cur` | vdi_mouse.c:127 | dis_cur (mouse.rs:300) | impl | **ja 1:1** |
| `vq_mouse` | vdi_mouse.c:341 | vq_mouse (mouse.rs:398) | impl | ja* (set_sr entfällt — kooperativ) |
| `vex_butv` | vdi_mouse.c:370 | vex_butv (mouse.rs:415) | **STUB** | nein (V6-002) |
| `vex_motv` | vdi_mouse.c:391 | vex_motv (mouse.rs:419) | **STUB** | nein (V6-002) |
| `vex_curv` | vdi_mouse.c:414 | vex_curv (mouse.rs:423) | **STUB** | nein (V6-002) |
| `vex_wheelv` | vdi_mouse.c:438 | vex_wheelv (mouse.rs:427) | **STUB** | nein (V6-002) |
| `vex_timv` | vdi_misc.c:100 | vex_timv (mouse.rs:433) | impl | ja* (Tickcal=20 ok; Vektor nie aufgerufen) |
| `cur_display` (save+draw) | vdi_mouse.c:877 | draw (mouse.rs:225) | impl | **nein** (V6-001 PEL-Roundtrip) |
| `cur_replace` (restore) | vdi_mouse.c:1066 | erase (mouse.rs:253) | impl | **nein** (V6-001) |
| `vdimouse_init` | vdi_mouse.c:565 | init/ensure_init (mouse.rs:185) | impl | **ja** (HIDE=1, Center, ARROW byte-exakt) |
| `v_locator` (vrq/vsm) | vdi_mouse.c:280 | v_locator (input.rs:98) | partial | **nein** (V6-003) |
| `v_choice` (vrq/vsm) | vdi_input.c:31 | v_choice (input.rs:124) | impl | **ja 1:1** (Dummy=1) |
| `v_string` (vrq/vsm) | vdi_input.c:79 | v_string (input.rs:136) | impl | ja* (256-Cap, V6-004) |
| `vsin_mode` | vdi_input.c:118 | vsin_mode (input.rs:25) | impl | **ja 1:1** |
| `vqin_mode` | vdi_input.c:154 | vqin_mode (input.rs:43) | impl | ja* (V6-005) |
| `v_valuator` (29) | vdi_main.c:61 (v_nop) | Op 29 nicht geroutet | impl | **ja** (EmuTOS selbst v_nop) |
| `vb_draw`/mouse_int | vdi_mouse.c:668, asm | feed_relative (mouse.rs:127) | adapter | HW-Adapter (kooperativer Poll) akzeptabel |
| PRESET_FORMS[0..7] | aes/mform.rsc | mouse.rs:65 | impl | **ja** (ARROW+TEXT byte-exakt @0xb4/@0x100) |

## Findings

### AUD-V6-001: Cursor-Save-under verlustbehaftet (PEL-Roundtrip statt bit-exakt) — VDI-Unterbau der Maus-Artefakte
- Severity: HIGH · CONFIRMED · latent
- EmuTOS: `cur_display` `*save++=*dst` (bit-exakt), `cur_replace` `*dst=*src++` (vdi_mouse.c:877,1066).
- NewTOS: `draw` `store[..]=read_pixel(px,py)` + `erase` `fill_rect(.., store[..])` (mouse.rs:238,264); `read_pixel` reverse-mappt XRGB8888 auf nächsten der **16** Pens (vdi_backend.rs:229), fill_rect→pel_to_rgb.
- Divergenz: Für jeden Pixel, dessen exakte FB-Farbe kein 16-Pen-Eintrag ist (Truecolor-Bilder/zView, Dither/Shade), speichert draw den nächsten Pen, erase restauriert dessen Palettenfarbe — nicht das Original. Ganze 16×16-Box betroffen → Farb-Korruptionsblock zieht mit dem Cursor. Backend hat `get_device_pixel`/`put_device_pixel` genau dafür (backend.rs:186, von bb_save genutzt); Cursor nutzt sie nicht.
- 1:1-Fix: draw/erase über `get_device_pixel`/`put_device_pixel` (+flush_rect) exakt sichern/restaurieren; Cursor-Anstrich bleibt fill_rect(fg/bg).
- **AES-P5-Bezug:** Für den palettenbasierten Desktop (VDI-Pen-Content) ist der Roundtrip visuell verlustfrei (best_d=0) → **NICHT** die Ursache der gemeldeten Desktop-Droppings (die ist AUD-P5-002, AES-Redraw ohne gsx_moff/gsx_mon). V6-001 ist latenter Gap, sichtbar bei Truecolor-Bildbetrachter. Runtime-verify: yes.

### AUD-V6-002: `vex_butv/motv/curv/wheelv/timv` — installierte Vektoren nie aufgerufen (partial STUB)
- Severity: MEDIUM · CONFIRMED
- EmuTOS: Vektoren aus mouse_int/wheel_int/vb_draw/tick_int aufgerufen; Init `user_but/mot=just_rts`, `user_cur=mov_cur` (vdi_mouse.c:585).
- NewTOS: USER_BUT/MOT/… (mouse.rs:37) + swap_vector (mouse.rs:408) — grep: kein Leser; feed_relative ruft weder user_mot noch user_but. Init-Return CONTRL[9..]=0 statt Adresse (App, die alten Vektor sichert, restauriert NULL).
- 1:1-Fix: feed_relative als Adapter soll bei Button-Change `user_but(MOUSE_BT)`, bei Bewegung `user_mot`/`user_cur` triggern; Slots auf just_rts-Äquivalent init. Real-Impact moderat (meiste Apps nutzen AES evnt_multi). Runtime-verify: yes.

### AUD-V6-003: `v_locator` setzt GCURX/GCURY nicht, keine dis_cur/hide_cur-Klammer, kein Button/Motion-Terminator
- Severity: MEDIUM · CONFIRMED
- EmuTOS: `GCURX=point->x; GCURY=point->y` unbedingt beide Modi (vdi_mouse.c:285); REQUEST klammert dis_cur/hide_cur; gloc_key terminiert auf Button ODER Taste, Motion via cur_ms_stat&0x20.
- NewTOS: input.rs:98-118 liest ptsin(0/1) lokal, schreibt nie GCURX/GCURY; kein dis/hide_cur; REQUEST blockiert nur auf read_key, kein cur_ms_stat → nie Button-Terminierung.
- Folge: folgendes vq_mouse meldet in EmuTOS den Locator-Punkt, in NewTOS die Live/Park-Position. Locator faktisch tastatur-only.
- 1:1-Fix: GCURX/Y am Eintritt setzen; REQUEST mit dis_cur/hide_cur; cur_ms_stat im Treiber pflegen + auswerten. Runtime-verify: yes.

### AUD-V6-004: `v_string` REQUEST-Cap bei 256 (EmuTOS ungecappt) + SAMPLE-Grenz-Consume
- Severity: LOW · CONFIRMED
- EmuTOS: REQUEST-Schleife bis i<j ohne Obergrenze; SAMPLE `while((gchr_key()!=0)&&(i<j))` — gchr_key vor i<j (vdi_input.c:89).
- NewTOS: MAX_STRING=256 cap (input.rs:142); SAMPLE Limit-Check vor Lesen (input.rs:166). Folge: >256 Zeichen truncated; SAMPLE-Puffer-Zustand differiert um 1. Selten (Felder ≤256). 1:1-Fix: Cap raus; gchr_key-vor-Limit.

### AUD-V6-005: `vqin_mode` Out-of-range → INTOUT[0]=0 statt stale
- Severity: LOW · CONFIRMED
- EmuTOS: switch ohne default → INTOUT[0] unverändert; Dispatcher preloadt INTOUT[0] nicht (vdi_input.c:154). NewTOS: mode=0→set_intout(0,0) (input.rs:47). Undefiniert in beiden, praktisch irrelevant. Runtime-verify: no.

## 1:1-Treffer (adversarial bestätigt, kein Finding)
HIDE_COUNT-Zählung 1:1 (dis_cur/hide_cur/v_show_c alle Fälle inkl. Underflow-Clamp/Force-Show, verschachtelt hide×3;show×2→HIDE=1); Save→draw/erase→restore-Reihenfolge korrekt (BACK.valid-Swap verhindert Doppel-Restore); Rand-Clipping sicher (read_pixel bounds-checked, fill_rect clamp_to, gleiche BACK.ox/oy); Preset-Formen byte-exakt gegen mform.rsc (ARROW hot 0,0; TEXT hot 7,7); Tickcal=20 (bios.c:1096); Dispatcher nptsout/nintout-Slots gegen jmptb1/jmptb2; v_choice 1:1; vsin_mode 1:1; v_valuator=v_nop 1:1.

## Akzeptable Abweichungen (API-unobservierbar)
MAP_COL weggelassen in vsc_form/set_form (PEL-Backend löst gleich auf, validate_color_index angewandt → identische Farbe); vq_mouse ohne set_sr (Feed kooperativ via usb::poll synchron, kein präemptiver IRQ → kein Torn-Read); vb_draw/mouse_int durch feed_relative ersetzt (HW-Adapter, Memory-konform).

## Zusammenfassung
- **Severity:** HIGH 1 (001) · MEDIUM 2 (002,003) · LOW 2 (004,005). + 3 akzeptable Abw. + 9 bestätigte 1:1. 19 Fn auditiert, 5 echte Gaps.
- **AES-P5-Bezug (Kern):** VDI-Cursor-Unterbau 1:1 sauber → Desktop-Maus-Artefakte auf AES-Schicht isoliert: **AUD-P5-002** (Redraw ohne gsx_moff/gsx_mon = Hauptursache) + **AUD-P5-006** (graf.rs nutzt forcierende remove/display_at statt zählender hide/show; beide existieren korrekt mouse.rs:354/363 vs 390/378). V6-001 tritt separat bei Truecolor hinzu.

**Top-3:** V6-001 (Save-under PEL-Roundtrip, latent Truecolor), V6-002 (vex_* nie aufgerufen), V6-003 (v_locator GCURX/Y + Klammer).

**OOS:** Modul-Doku mouse.rs:6-9 („not yet wired … parked cursor") ist stale — feed_relative real verdrahtet via ikbd.rs:121.
