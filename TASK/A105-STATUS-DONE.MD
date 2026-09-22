# A105 — STATUS: DONE (Hostbeleg rot/gruen + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen (Punkt 1)

Die vier Rahmen-Restposten nach A95 sind geschlossen: der `W_INFO`-Text
sitzt auf Zelle (1,25) des Rahmens, der Name wird nach `gr_just` auf der
gewachsenen Zelle zentriert (und bei Ueberlaenge linksbuendig gekuerzt),
`thumb_geom` nimmt je Achse das richtige Mindestmass, und ein gedruecktes
Rahmen-Gadget folgt `changecol` (`actcolchange`). Bearbeitet 2026-09-21,
Branch `desk-rebuild`, Basis HEAD `a570123`, **nicht committet**, nichts
geflasht. Gemeinsames Paket mit A103/A108
(`measurement-2026-09-21-a103-a108/`).

## Der EmuTOS-Pfad (`references/emutos/aes/`)

| Was | Zeilen |
|---|---|
| `gl_asamp`: `TE_LEFT`, `te_thickness 1` (`gl_ainfo`); `gl_aname.te_just = TE_CNTR`; `te_ptext` = Name nur bei `NAME`, sonst `""` | `gemwmlib.c:154-157, 501-503` |
| `W_INFO` `FL3DNONE`, `W_NAME` `FL3DACT` bei `NAME` | `gemwmlib.c:110-129, 553` |
| 3D-Wachstum um `ADJ3DSTD`, `movetext`/`changecol` = `acttxtmove`/`actcolchange` fuer `FL3DACT`, sonst FALSE / `!xor_is_ok` | `gemoblib.c:503-524` |
| `G_BOXTEXT`-Text: `c = gr_inside(&t, tmpth)`, `movetext` → SELECTED `x++` sonst `y--`, `gr_gtext(te_just, te_font, te_ptext, &c)` | `gemoblib.c:735-757` |
| `gsx_tcalc` (Breite/Hoehe an die Zelle geklemmt, `numchs = min(n, textw/wc)`, 0 unter Glyphhoehe); `gr_just` (`(slack+1)/2` je Achse nur bei Slack; TE_LEFT bleibt am Ursprung) | `gemgraf.c:480-497, 567-589` |
| `changecol && SELECTED`: hohl → `IP_SOLID` schwarz, sonst `icol = xor_color(icol)`; `tcol = xor_color(tcol)`, `MD_TRANS`; `add_3d_effect(…, icol)` mit dem geaenderten `icol` | `gemoblib.c:681-690, 699-706, 950`; `xor_color`/`vdi16comp` `:83-102` |
| Standardwoerter `0x1101` (hohl, `ic = BLACK`): keine 3D-Flaechensubstitution (`:619-636` verlangt hohl **und** weiss) | `gemwmlib.c:1346-1349` |
| Elevator: vertikal `gl_hbox`, horizontal `gl_wbox` | `gemwmlib.c:431-434, 469-471` |

## Referenzpixel (`falcon-tc-window-c.png`, Fenster (16,96,304,192), `dumpwin.py … 0 22 120 43`)

| Punkt | Referenz (rahmenrelativ) | NewTOS vorher |
|---|---|---|
| 1 | Infozeile ` 514 bytes used in 1 items.`: `5` (Zelle 1) Spalten 10..15, Zeilen 27..38; `y` (Zelle 6) Spalten 50..55, Zeilen 30..40; Zeilen 24..26 weiss → Zellen ab (1,25) | Zellen ab (3,24): `5` bei 12..17 / 26..37 |
| 2 | (kein Referenzbild mit ungerader Breite; Formel) Breite 321: Name „A A" Zelle x = 179; 40 Zeichen bei Breite 320: 34 Glyphen ab Spalte 24 | 178; 33 Glyphen ab Spalte 27 (Tinte 58..319 statt 55..324) |
| 4 | (per `objc_sysvar`, kein Referenzbild) `actcolchange = 1`: Innen schwarz, Glyphe weiss, Fase schwarz/LWHITE | Innen weiss, Glyphe schwarz (unveraendert) |

## Was gebaut ist

* **`tos-api/src/aes/window.rs`** — `TE_LEFT`/`TE_CNTR`; `gr_just_text(just,
  c, n) -> Option<(x, y, numchs)>` (= `gsx_tcalc` + `gr_just`); `draw_label`
  zeichnet ab der Zellenoberkante (kein `+1` mehr) genau `numchs` Glyphen.
  Namens-Arm: `c = inside_1(name_box)`, `c.y -= 1` bei `NAME && ACT_TXTMOVE`,
  Text nur bei `NAME`, `label_walk(c.w)`. Info-Arm: `c = inside_1(info_box)`,
  `TE_LEFT`. `thumb_geom(track_len, slsize, slpos, min_size)` mit `gl_hbox()`
  / `GL_WBOX` an den fuenf Aufrufstellen. `gadget_sel`: `ACT_COLCHANGE && sel`
  → Innen/Glyphe/Modus wie `just_draw`, `add_3d_effect` mit dem geaenderten
  `ic`. Konstante `BLACK`. Unit-Test `thumb_geom_takes_the_axis_minimum`
  (19 vs 23).
* **`tos-api/src/aes/object.rs`** — nur Sichtbarkeit: `ACT_COLCHANGE`,
  `xor_color` → `pub(crate)`.
* **`tos-api/tests/aes_a105_frame_remnants.rs`** (neu, 5 Faelle; Harness aus
  `aes_a100_a102_…` mit `snap_held`, Pixelhelfer aus
  `aes_window_frame_reference.rs`, Glyph-Bboxen aus `vdi::font::glyph`).
* **`tos-api/tests/aes_window_frame_reference.rs`** — `name_cells` auf die
  `gr_just`-Formel (gerade Referenzbreite: unveraendert, 6/6 gruen).
* **`tos-api/tests/aes_window_color_words.rs`** — `FRAME_HASH`
  `0xb3e8_5614_b4ca_2900 → 0xfb9c_ac5d_6a24_ab40`, Begruendung im Kommentar
  (Scratch-Diff 2160 px, Bbox (43,86)-(385,200), alle in den beiden
  Infozeilen, 1080 K→W + 1080 W→K; stabil x2 und `--shuffle-seed 7/99`).
* **`docs/api/aes/window-graphics.md`** — Absatz „Text placement in
  `W_NAME`/`W_INFO`, the pressed `ACT` gadget (A105)".

## Tests: rot vor, gruen nach (`host-tests/`)

| Test | vor (HEAD `a570123`) | nach |
|---|---|---|
| `a105_1_info_text_starts_one_column_inside_the_border_on_the_third_row` | rot: `5`-Bbox (28,32,122,133) statt (26,31,123,134) — zwei rechts, eine hoch | gruen |
| `a105_2_odd_width_name_is_centred_with_gr_justs_rounding` | rot: Tinte 179..200 statt 180..201 | gruen |
| `a105_2_overlong_name_is_left_aligned_and_cut_to_whole_glyphs` | rot: 58..319 statt 55..324 | gruen |
| `a105_4_pressed_closer_keeps_its_colours_without_actcolchange` | gruen (Standard unveraendert) | gruen |
| `a105_4_pressed_closer_takes_the_complement_colours_with_actcolchange` | rot: Innen (32,42) weiss statt schwarz | gruen |
| `window.rs::thumb_geom_takes_the_axis_minimum` | (Signatur neu, an HEAD nicht uebersetzbar) | gruen |

`cargo run -p xtask -- test` **811 / 0 / 1 → 821 / 0 / 1** (mit A103/A108).

## QEMU raspi4b (`screendumps/`, `PIXDIFF.TXT`)

C:\-Fenster (48,43,544,400) im Idle-Dump gegen `falcon-tc-window-c.png` mit
A95s `corner_residual.py`: TL/TR/BR **0/0/0** vorher wie nachher (das
Infozeilen-Innere bleibt als Inhalt maskiert — die Strings sind verschieden,
EmuDESK ` 514 bytes used in 1 items.` vs unser ` 24 items   373645 bytes
used`). Die **Lage** des Infotexts: Referenz Tinte Zeilen 27..40, erste
Spalte 10 (Zelle 1), Bbox der ersten Glyphe zellenrelativ (1,6,2,13);
nachher identisch; vorher 26..39 ab 12, (3,7,1,12). Vorher/nachher-Diff des
Idle-Dumps 2084 px = Leistentitel (A108) + Infozeile (x ab 58, Zeilen 66..83).

## Nicht angefasst (Nebenbefunde)

* Die Scroll-Pfeile haben in NewTOS kein Druck-Feedback (EmuTOS
  `ob_change(gadget, selected)` um `handle_arrow_msg`, `gemctrl.c:272-284`);
  `changecol` gilt damit praktisch fuer Closer/Fuller (`gr_watchbox`). Eigener Task.
  **Erledigt 2026-09-22 als [A109](A109.md)** (2) — `arrow_change` klammert
  `wait_release`, Seitenklicks bleiben ohne Feedback (EmuTOS' Wache).
* Das Infozeilen-Format des Kernel-Desktops weicht von EmuDESK ab (desk/-Inhalt).
  **Erledigt 2026-09-22 als [A109](A109.md)** (3) — `STINFOST`.

## Offen

Pi-Sichtabnahme fuer Punkt 1 (Tester/Lead; nichts geflasht). Kein Commit.
