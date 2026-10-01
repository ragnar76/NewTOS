# A95 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen

Der Fensterrahmen ist jetzt in den drei A79-Ecken pixelgleich mit EmuTOS auf
dem Hatari-Falcon (`falcon-tc-window-c.png`), bis auf Inhalt, Zeiger,
Elevatorlaenge und die bildschirmausgerichtete Musterphase; alle sechs
Deltas des Tickets sind geschlossen. Bearbeitet 2026-09-21, Branch
`desk-rebuild`, Basis HEAD `06bb448`, **nicht committet**, nichts geflasht.

## Der EmuTOS-Pfad (`references/emutos/aes/`)

| Was | Zeilen |
|---|---|
| `add_3d_effect`: nach `gr_inside(&r, th)` zwei Polylines — `(x,y+h-2)→(x,y)→(x+w-2,y)` WHITE (SELECTED: BLACK), `(x+1,y+h-1)→(x+w-1,y+h-1)→(x+w-1,y+1)` LBLACK (SELECTED: LWHITE; BLACK/WHITE, wenn gleich `icol` oder jenseits `ws_ncolors`) — 1 px, beide Fernecken frei; zuletzt in `just_draw` fuer jedes `FL3DOBJ` | `gemoblib.c:231-237, 238-276, 252, 270-272, 949-950` |
| 3D-Wachstum um `ADJ3DSTD`, `movetext = acttxtmove` (Default 1) | `:503-513, 1332` |
| `G_BOXCHAR`-Glyphe: `c = gr_inside(&t, tmpth)`, SELECTED → `c.g_x++`, sonst `c.g_y--`, `gr_gtext(TE_CNTR, IBM, ch, &c)` | `:735-757` |
| `gr_just`: `gsx_tcalc` (`w = min(c.w, 8)`, `h = min(c.h, 16)`, kein Zeichen unter Glyphhoehe), `y += (h+1)/2`, `x += (w+1)/2` → Zelle (8,3) im 23x23-Slot | `gemgraf.c:480-497, 565-585` |
| `DISABLED`: `bcol = WHITE` (FL3DACT), `bb_fill(MD_TRANS, FIS_PATTERN, IP_4PATT, t)` auf dem Innenrechteck, **vor** `add_3d_effect` | `gemoblib.c:889, 917-930` |
| `w_bldactive`: `W_NAME.ob_state = istop ? NORMAL : DISABLED` (beide Arme), `W_NAME` `FL3DACT` genau bei `NAME` | `gemwmlib.c:495, 537, 546, 553` |
| `W_VBAR` `(x, 0, adj_wbox, work_h+2)` bis zur Naht; `w_bldvbar`: UPARROW `(ADJ3DSTD, ADJ3DSTD)`, je Pfeil `y/h -= adj_hbox`, DNARROW `(ADJ3DSTD, y+h+ADJ3DSTD-1)`, VSLIDE `(0, y, adj_wbox, h)`, VELEV `(ADJ3DSTD, posn+ADJ3DSTD, gl_wbox, size-2*ADJ3DSTD-1)` | `:585-610, 401-436` |
| `W_HBAR` `(0, y, work_w+2, adj_hbox)` ab der linken Randspalte; `w_bldhbar`: LFARROW `(x+ADJ3DSTD, ADJ3DSTD)` mit `x = 0`, RTARROW `(x+w-1+ADJ3DSTD, ADJ3DSTD)`, HSLIDE `(x, 0, w, adj_hbox)` | `:612-616, 441-474` |
| `ob_find`: `FL3DOBJ` antwortet fuer den gewachsenen Slot (`ob_actxywh`); Kinder ab `ob_tail` rueckwaerts → die Spur (nach den Pfeilen eingehaengt) besitzt die geteilte Randlinie | `gemoblib.c:1034-1113, 313-338`; `gemwmlib.c:427, 465` |

## Referenzpixel (mit `method/dumpwin.py` gelesen; `falcon-tc-window-c.png`, Fenster (16,96,304,192))

| Delta | Referenz (rahmenrelativ) | NewTOS vorher |
|---|---|---|
| 1 | Closer LBLACK nur Spalte 21 (Zeilen 2..21) / Zeile 21 (Spalten 2..21); Spalte 20, Zeile 20 weiss; (1,21), (21,1) weiss. Fuller Spalte -2 / Zeile 21, Up-Pfeil Spalte -2 (44..63) / Zeile 63, Sizer Spalte -2 / Zeile -2, Elevator Spalte -2 / Zeile 81 | 2 px (Spalten 20..21, Zeilen 20..21) |
| 2 | Namenskasten: Zeile 1 weiss x=40..294, Spalte 40 weiss 97..116; Zeile 117 LBLACK 41..295, Spalte 295 LBLACK 98..117; (295,97), (40,117) = Stipple | kein Bevel, Stipple bis Zeile 1/21, Spalte 24/-25 |
| 3 | Tinten-Bbox: Closer 8..14 / 5..15; Fuller -15..-9; Up -15..-8 / 48..57; Sizer -15..-9 / -18..-8; Links 8..15 / -18..-9; Down -15..-8 / -40..-31; Rechts -38..-31 / -18..-9 | je eine Spalte weiter links (Closer 7..13; Links-Pfeil gleich, weil sich (3) und (5) dort aufheben). Die Ticketzahlen „7..14 vs 6..13" sind beidseitig um eins verschoben. |
| 4 | Zeile -47 `IP_1PATT`-Spurinneres; Zeilen -46, -24, -23 schwarz ueber die Balkenspalte | Slot -47..-25, weisse Zeile -24, Spur eine Zeile kuerzer |
| 5 | Zeilen -22..-2: Spalten 22 und 23 schwarz, 21 LBLACK, 20 weiss; Spalten -24, -23 schwarz, -25 LBLACK, -26 weiss, -46 schwarz | Links 1..23, Rechts -47..-25 (weisse Spalte -24) |
| 6 | `falcon-tc-two-windows.png`: (186,69) schwarz, (187,69) weiss, (187,70) schwarz; Zeile 85 massiv LBLACK; Closer massiv | Name massiv |

## Was gebaut ist

* **`tos-api/src/aes/window.rs`**
  * `gadget_sel`: `draw_box_ink(slot)`, Glyphe bei `boxchar_origin(slot, sel)`,
    `object::add_3d_effect(slot, sel, th = 1, ink.ic)`. `bevel()` (2-px-Band)
    entfernt, `GRAY_DK` (tot) entfernt. `draw_elevator` und die glyphlose
    Sizer-Ecke ebenso auf `add_3d_effect`.
  * `boxchar_origin` (neu): `gr_inside(slot, 1)`, `ACT_TXTMOVE`-Nudge
    (SELECTED x+1, sonst y-1), `gsx_tcalc`/`gr_just`-Rundung; `None`, wenn die
    Zelle keine Glyphe fasst.
  * Namens-Arm in `draw_frame`: nach dem Label bei `!istop`
    `fill_pattern(IP_PATTERNS[IP_4PATT], WHITE, Transparent)` auf
    `inside_1(name_box)` (beide Arme, `:537/:546`); danach bei `NAME`
    (`FL3DACT`) `add_3d_effect` auf dem gewachsenen Namenskasten.
  * `vtrack`/`htrack`: auf `vbar_box`/`hbar_box` (EmuTOS `W_VBAR`/`W_HBAR`)
    statt `vbar_rect`/`hbar_rect` — vertikale Spur eine Zeile laenger
    (Unterkante = Oberkante des Down-Slots), horizontale beidseitig eine
    Spalte breiter, Elevator-Weg = `h - size`.
  * `varrow_cells`/`harrow_cells` (neu): die Pfeilzellen nach
    `w_bldvbar`/`w_bldhbar`; `draw_varrows`/`draw_harrows` benutzen sie.
  * `gadget_at`: Pfeilzellen wie `ob_find` auf den gewachsenen Slots,
    `ob_tail` zuerst — Dn ab der Zeile unter der Slot-Oberkante (vorher
    genau diese), Rt ab der Spalte rechts der Slot-Linken (vorher diese),
    Lf bis zur Slot-Rechten (vorher eine weiter); Up gleich. Doc-Kommentar
    mit den Zeilen. `vbar_rect`/`hbar_rect` (Rechteckliste/Trefferspalte),
    `wm_calc`, Rechtecklisten unveraendert.
* **`tos-api/src/aes/object.rs`** — nur Sichtbarkeit: `add_3d_effect`,
  `IP_PATTERNS`, `ACT_TXTMOVE` → `pub(crate)`; neue Konstante
  `pub(crate) const IP_4PATT: u16 = 4`. Kein Verhaltenswechsel.
* **`tos-api/tests/aes_window_frame_reference.rs`** (neu, 6 Faelle, Harness
  aus `aes_window_color_words.rs`): `bevel_is_one_pixel_on_every_fl3dact_gadget`,
  `topped_name_box_carries_the_fl3dact_bevel`,
  `gadget_glyphs_are_centred_in_the_grown_cell`,
  `down_arrow_slot_ends_on_the_row_above_the_seam`,
  `horizontal_arrow_slots_sit_on_the_band_edges`,
  `untopped_name_is_drawn_disabled` — jeder Doc-Kommentar nennt die
  kodierten Referenzpixel.
* **`tos-api/tests/aes_window_color_words.rs`** — T7, `default_topped`,
  `default_untopped` auf die Referenzgeometrie nachgezogen (Spur = `W_VBAR`
  minus zwei Pfeile; Links-Pfeil bei `x = 0`; Fasenring des Namenskastens
  behauptet, Farbaussagen innerhalb); `FRAME_HASH`
  `0xaf7b81ef83fec1a0 → 0xb3e85614b4ca2900`, Begruendung im Kommentar
  (Scratch-Diff 2859 px, alle in Gadgetzellen/Namenskaesten von h1/h2 plus
  h1's zuerst gezeichnete rechte untere Ecke unter h2's nie gemalter
  Arbeitsflaeche; K→W 909, LK→W 674, W→LK 505, W→K 428, K→LK 255, LK→K 88;
  nichts ausserhalb der beiden Rahmen; stabil allein x2 und unter
  `--shuffle-seed 7/99 --test-threads=1`).
* **`tos-api/tests/aes_window_open_oldtop.rs`** — `assert_untopped_title`:
  Fasenring behauptet, Hohl-Weiss-Aussage innerhalb.
* **`docs/api/aes/window-graphics.md`** (2): „2px bevel" → 1-px-Polylines
  mit `gemoblib.c:238-276, 949-950`, `gemwmlib.c:553` (korrigiert, A95).
* **`docs/STATUS.md`** — Phase-13-Zeile (Eintrag 2026-09-21 (A95)) + eine
  Zeile „Recent Decisions" 2026-09-21.
* **`measurements/measurement-2026-09-21-a95/`** — `RUNINFO.TXT`, `manifest.sha256`,
  `manifest-scratch.sha256`, `method/` (A94-Skripte + `dumpwin.py`,
  `corner_compare.py`, `corner_residual.py`, `README.md`,
  `scratch-host_env.rs.txt`), `host-tests/`, `build/`, `screendumps/`,
  `serial/`. Keine `*.img`.

## Tests: rot vor, gruen nach

`host-tests/aes_window_frame_reference-RED-before-fix.txt` / `-GREEN-after-fix.txt`:

| Test | vor (erste Zeile) | nach |
|---|---|---|
| `bevel_is_one_pixel_on_every_fl3dact_gadget` | „W_CLOSER: bottom-left corner is untouched face: left 9 (LBLACK) right 0" | gruen |
| `topped_name_box_carries_the_fl3dact_bevel` | „W_NAME bevel top (ref row 97): pixel (55,41): left 1 right 0" | gruen |
| `gadget_glyphs_are_centred_in_the_grown_cell` | „W_CLOSER glyph ink bounding box: left (37,43,45,55) right (38,44,45,55)" | gruen |
| `down_arrow_slot_ends_on_the_row_above_the_seam` | „W_DNARROW border top row: pixel (328,234): left 0 right 1" | gruen |
| `horizontal_arrow_slots_sit_on_the_band_edges` | „W_LFARROW border right column: pixel (52,258): left 9 right 1" | gruen |
| `untopped_name_is_drawn_disabled` | „untopped W_NAME bevel bottom (LBLACK): pixel (35,61): left 0 right 9" | gruen |

`cargo run -p xtask -- test` **788/0/1 → 794/0/1**
(`host-tests/xtask-test-after-fix.log`). Frischer `cargo xtask build --board
rpi4` (nach `cargo clean -p kernel -p tos-api --target
aarch64-unknown-none-softfloat`): **tos-api 64 / kernel 94**, rc 0, die
Warnungsmenge ist zeilengleich mit A94's Log (`build/xtask-build-rpi4.log`;
eine voruebergehende „`GRAY_DK` never used" wurde durch Entfernen der toten
Konstante geschlossen). `cargo clippy -p tos-api`: **80**, Menge identisch
mit A94 (`build/clippy-tos-api-host.txt`); die drei beruehrten Testdateien
0 Treffer (`build/clippy-tos-api-tests.txt`). Kein qemu-virt-Build
(Lead-Anweisung).

## QEMU raspi4b (Selbstbeleg, kein Testerurteil)

A94-Methode (`method/`, Scratch-Kernel mit Injektor — `window.rs` im Scratch
== Baum, per `diff` geprueft; `parkbr` vor jedem Dump; 0 Exceptions in
beiden Serial-Logs; Settle-Diffs 0 px):

* **TeraDesk `#Z`** (sd-tera, `tera-two.actions`): `tera-B-one.png`, das
  C:\*.*-Fenster (39,62,376,291). **Der definitive Vergleich**
  (`screendumps/final-compare-ref-vs-newtos-a95.png`, oben Referenz, unten
  wir; dieselben Crops wie A79: TL (0,0)+120x48, TR (w-72,0)+72x48, BR
  (w-72,h-72)+72x72, 4x): Rohdiff der Klassen TL 1603 / TR 424 / BR 199 —
  davon nicht Rahmen: Namensband-Musterphase 1280 (die VDI richtet
  `IP_2PATT` an absoluten Bildschirmkoordinaten aus, `vdi/vdi_fill.c`; die
  Referenz beginnt bei x=16, wir bei x=39 — jedes dieser Pixel entspricht in
  beiden Bildern der Erwartung der eigenen Bildschirmposition, auch die zwei
  freien Fasenecken (24,21)/(-25,1)), Infozeilentext 747 (EmuDESK „ 514 bytes
  used in 1 items." vs TeraDesks Zeile), Arbeitsflaeche 48, Spurinneres 137
  (`IP_1PATT`-Zeilenphase (96+145) mod 4 = 1 vs (62+244) mod 4 = 2, und
  Elevatorlaenge), Zeiger 14 (Referenz, rechter Rand, Zeilen -50..-36) →
  **Rest 0 Pixel in allen drei Ecken**
  (`host-tests/corner-residual-topped-tera-vs-ref.txt`). Diff gegen A94
  `tera-B-one`: 1832 px, Bbox (40,63)-(414,352) = innerhalb des Rahmens.
* **Kernel-Desktop** (sd-base, `desk.actions`): `desk-B-two.png`, hinteres
  (ungetopptes) Fenster (48,43,544,400). Titelband TL/TR (Zeilen 0..21; der
  Rest liegt unter dem vorderen Fenster) gegen `falcon-tc-two-windows.png`
  (16,64,384,320): **Rest 0 Pixel**
  (`host-tests/corner-residual-untopped-desk-vs-ref.txt`); der Name „C:\"
  ist das `IP_4PATT`-Schachbrett der Referenz (Zeilen 5..16 abwechselnd
  `#.#.`/`.#.#`, 42 Tintenpixel), Zeile 21 massiv LBLACK, Closer massiv
  (`screendumps/compare-untopped-title-ref-vs-newtos-a95-4x.png`). Diff gegen
  A94 `desk-B-two`: 2925 px, Bbox (49,44)-(615,466) = innerhalb der beiden
  Rahmen; `desk-A-idle` gegen A94: 2129 px innerhalb des Rahmens;
  `tera-A-idle` 0 px (noch kein Fenster).

## Beobachtungen ausserhalb des Umfangs (unveraendert)

* **`W_INFO`-Textursprung** (im Eckenvergleich als Inhalt maskiert, beim
  Sichtvergleich des TL-Crops aufgefallen): EmuTOS `gl_ainfo` ist `TE_LEFT`
  mit `te_thickness 1` (`gl_asamp`, `gemwmlib.c:154-157`), `FL3DNONE` (kein
  Wachstum, `movetext` FALSE `gemoblib.c:521-524`); `gr_gtext` bekommt
  `c = gr_inside(&t, 1) = (1, 24, w-2, 17)` (`:737-738`), `gr_just` addiert
  `(17-16+1)/2 = 1` (`gemgraf.c:574-576`) → Zelle (1,25), Zeilen 25..40
  (Referenz: erste Tintenzeile einer Ziffer 27, `y`-Unterlaenge 39/40).
  NewTOS `draw_label(b.x + 3, top, …)` mit `cy = y + 1` → Zelle (3,24),
  Zeilen 24..39 (`tera-B-one.png`: erste Tintenzeile 26). Zwei Spalten
  rechts, eine Zeile hoch. Folgeticket.
* Namenstext-Zentrierung: NewTOS `name_x + ((name_w - nlen) / 2).max(2)`
  auf der ungewachsenen Zelle; EmuTOS `gr_just` auf `gr_inside(gewachsen, 1)`
  mit `(w+1)/2` und Linksbuendigkeit ab Spalte 24 bei Ueberlaenge — gleich
  fuer gerade Fensterbreiten (Referenz und Dumps), eine Spalte Unterschied
  bei ungeraden. Folgeticket.
* `thumb_geom` nimmt fuer beide Achsen `gl_hbox` als Mindest-Elevator;
  EmuTOS `w_bldhbar` nimmt horizontal `gl_wbox` (`gemwmlib.c:469-470`) —
  auf diesem Profil gleich (19/19).
* `changecol` (`actcolchange`, Default 0) fuer gedrueckte Rahmen-Gadgets ist
  nicht modelliert (nur `movetext` und die SELECTED-Fasenfarben); ohne
  `objc_sysvar`-Aenderung unbeobachtbar.

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht). Kein Commit.
