# A94 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen

`wind_open` eines zweiten Fensters zeichnet das alte Top-Fenster jetzt
ungetoppt („covered") neu — durch seine **eigene** Rechteckliste, also in
jedem sichtbaren Stueck seines Rahmens, auch ausserhalb des neuen Fensters
und auch ohne Ueberlappung — wie EmuTOS' `draw_change` im Oeffnen-Fall.
Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis HEAD `f1b916f`, **nicht
committet**, nichts geflasht.

## Der EmuTOS-Pfad (`references/emutos/aes/gemwmlib.c`)

| Schritt | Zeilen |
|---|---|
| `wm_open`: `VF_ISOPEN`, `w_obadd(W_TREE, ROOT, w_handle)` (neues Fenster = `ob_tail`), `draw_change(w_handle, &t)`, danach `w_setsize(WS_PREV, w_handle, pt)` | `:1410-1432` (`:1424-1427`) |
| `draw_change`: `c` = altes `WS_CURR` (= `gl_rzero` nach `wm_create` `:1377` bzw. `wm_close` `:1449,1073`), `WS_PREV := c`, `WS_CURR := pt`, `WS_WORK` | `:1069-1077` |
| Rechtecklisten neu (`newrect`) mit der neuen Ordnung | `:1088` |
| `oldtop = gl_wtop; gl_wtop = W_TREE[ROOT].ob_tail; w_setactive()` | `:1090-1094` |
| „a grow that is a superset or an open": `c` null → `rc_copy(pt, &c)`, `start` bleibt `w_handle` | `:1156-1167` |
| „update gl_wtop after close, or open": `oldtop != ob_tail` → Union mit `WS_CURR(gl_wtop)` (no-op); **`if (oldtop != NIL && oldtop != w_handle)`, `WS_PREV(gl_wtop) == gl_rzero` → `w_cpwalk(oldtop, 0, MAX_DEPTH, TRUE)`** — kein Ueberlappungstest | `:1194-1209` (`:1203-1207`) |
| `w_cpwalk(…, TRUE)`: Clip = `WS_TRUE(oldtop)`, `w_bldactive(oldtop)`, `do_walk` ueber `w_rlist` des alten Tops | `:363-379`, `:286-312` |
| `w_bldactive`: `istop = (gl_wtop == w_handle)` → false; `W_NAME` `DISABLED`; `w_setcolor` nimmt `w_bcolor` | `:495`, `:537`, `:382-397` |
| Schatten dazu, kein `w_drawdesk` (`start != DESKWH`), `w_update(w_handle, &c, DESKWH, FALSE)`: `bottom == top` → nur der Rahmen des neuen Fensters (`:1029`) + dessen `WM_REDRAW` (`:1030`); dessen Arbeitsbereich bleibt der Nachricht ueberlassen | `:1213-1222`, `:1014-1043` |

Die A79-Quittung zitierte „`:1202-1209`" — derselbe Block (`:1194-1209` in
der gepinnten Quelle); der Aufruf steht in `:1207`. Vorher lief bei uns nur
`redraw_clip(plus_shadow(r))` = `compose` ueber das neue Rechteck + Schatten:
das alte Top wurde nur *innerhalb* dieses Rechtecks ungetoppt neu gebaut,
ausserhalb blieben Stipple und getoppte Fase stehen.

## Was gebaut ist

* **`tos-api/src/aes/window.rs`** `wind_open`: `old_top = get_top()` vor dem
  Eintritt des Fensters in den Baum, `old_curr` vor dem Geometrie-Store; nach
  `w.z = bump_z()` und vor `redraw_clip`: wenn `old_top != DESKWH && old_top
  != handle && old_curr == gl_rzero` → `cursor_hide`;
  `draw_frame_pieces(old_top, false, backend, None)` (= `w_cpwalk(oldtop, 0,
  MAX_DEPTH, TRUE)`: `istop` false, eigene Rechteckliste, `WS_TRUE`-Clip);
  `cursor_show`. Sonst nichts geaendert. Kommentar mit den Zeilen oben.
* **`tos-api/tests/aes_window_open_oldtop.rs`** (neu, 3 Faelle; Harness aus
  `aes_window_color_words.rs`):
  `second_open_repaints_the_old_top_untopped_outside_the_new_window`
  (Teilueberlappung: h1's Titelband liegt ganz ausserhalb von h2 → hohl
  weiss, kein Stipple, Glyphen nur schwarz/weiss; Closer mit Rand, Fase,
  weisser Flaeche; h2's Band gestippelt; h2's Infoband dort, wo h1's
  Balkenspalte es kreuzt, weiss),
  `non_overlapping_second_open_still_repaints_the_old_top_untopped`
  (nebeneinander: dasselbe — `:1203-1206` kennt keinen Ueberlappungstest),
  `fully_covered_old_top_paints_nothing_outside` (Waechter: h1 ganz unter h2
  inkl. Schatten → leere Liste; ausserhalb von h2's Arbeitsbereich bildgleich
  mit „h2 allein", h2's Infoband ueber h1's Oberkante weiss, h2's Band
  gestippelt). Der `DISABLED`-Overlay (A95) wird nicht behauptet.
* **`tos-api/tests/aes_window_color_words.rs`** — `FRAME_HASH` neu gepinnt
  `0x259edc0b844d91a0 → 0xaf7b81ef83fec1a0`, Begruendung im Kommentar:
  Scratch-Diff der beiden Renderings von `default_frames` = 1236 Pixel, alle
  im Namensband von h1 (x 65..375, y 61..81), alle auf ungeraden
  Zeilen/Spalten = die `IP_2PATT`-Tinte des getoppten Worts `0x11a1`,
  schwarz → weiss; sonst nichts. Stabil allein x2 und unter
  `--shuffle-seed 7/99 --test-threads=1`.
* **`docs/api/aes/window-graphics.md`** — `wind_open`-Eintrag: Absatz „The
  previous top is repainted deactivated, in full" mit den Zitaten oben und
  dem Vertrag (jedes sichtbare Stueck, auch ohne Ueberlappung, nichts unter
  dem neuen Fenster; `w_update` nur fuer das neue Fenster); die A79-Restliste
  nennt den `wind_open`-Repaint nicht mehr (→ A94).
* **`docs/STATUS.md`** — Phase-13-Zeile (Eintrag 2026-09-21 (A94)) + eine
  Zeile „Recent Decisions" 2026-09-21.
* **`measurements/measurement-2026-09-21-a94/`** — `RUNINFO.TXT`, `manifest.sha256`,
  `method/` (A79-Skripte + `tera-two.actions`, `final_scripts.rs`,
  `scratch-host_env.rs.txt`), `host-tests/`, `build/`, `screendumps/`,
  `serial/`. Keine `*.img`.

## Tests: rot vor, gruen nach

`measurements/measurement-2026-09-21-a94/host-tests/aes_window_open_oldtop-RED-before-fix.txt`
/ `…-GREEN-after-fix.txt`:

| Test | vor | nach |
|---|---|---|
| `second_open_repaints_the_old_top_untopped_outside_the_new_window` | „h1 after h2 opened: untopped W_NAME is hollow white outside the glyphs, at (65,61): left 1 (BLACK) right 0 (WHITE)" | gruen |
| `non_overlapping_second_open_still_repaints_the_old_top_untopped` | „… at (35,41): left 1 right 0" | gruen |
| `fully_covered_old_top_paints_nothing_outside` | gruen (Waechter) | gruen |

`cargo run -p xtask -- test` **785/0/1 → 788/0/1**
(`host-tests/xtask-test-after-fix.log`). Frischer `cargo xtask build --board
rpi4` (nach `cargo clean -p kernel -p tos-api --target
aarch64-unknown-none-softfloat`): **tos-api 64 / kernel 94**, rc 0, Delta 0
(`build/xtask-build-rpi4.log`). `cargo clippy -p tos-api`: **80** (A79-Quittung
81; im beruehrten Bereich nur der vorbestehende `Aespb`-Sichtbarkeitshinweis
auf `wind_open`), die zwei Testdateien 0 Treffer
(`build/clippy-tos-api-*.txt`). Kein qemu-virt-Build (Lead-Anweisung).

## QEMU raspi4b (Selbstbeleg, kein Testerurteil)

A79-Methode (`method/`, Scratch-Kernel mit Injektor — `window.rs` im Scratch
== Baum, per `diff` geprueft; `parkbr` vor jedem Dump; 0 Exceptions in beiden
Serial-Logs):

* **Kernel-Desktop** (sd-base, `desk.actions`: Doppelklick auf das
  „C Hard disk"-Symbol → zweites C:-Fenster versetzt): `desk-B-two.png`.
  Diff gegen A79 `final-desk-B-two.png` (vorher): **2384 px, Bbox
  (73,45)-(567,63) = das Namensband des hinteren Fensters, alle schwarz →
  weiss, alle auf ungeraden Zeilen/Spalten = das Stipple; sonst nichts.**
  Schwarz im Bandinneren 2202 → 84 (= die Glyphen). `desk-A-idle` 0 px gegen
  A79; Settle `B` vs `C-2s` 0 px.
* **TeraDesk `#Z`** (sd-tera, `tera-two.actions`): der zweite Doppelklick
  auf HARD DISC C (195,35) oeffnet ein **zweites** C:\*.*-Fenster (TeraDesks
  zweiter INF-`pos`, x=497, teils ausserhalb des Schirms) — der Injektor kann
  zwei Fenster oeffnen. Altes Top: odd/odd-Schwarz im Band 1313 (ein Fenster,
  getoppt) → 47 (nach dem zweiten; Glyphentinte); neues Fenster 522 (Stipple).
  `tera-B-one` 0 px gegen A79 `final-tera-B-window`; Settle 0 px.
* **Crops 4x gegen die Hatari-Falcon-Referenz** (`falcon-tc-two-windows.png`,
  hinteres Fenster bei (16,64)): `screendumps/compare-oldtop-title-ref-before-
  after-4x.png` (Referenz | vorher | nachher) und `compare-tera-oldtop-title-
  ref-one-two-4x.png`. Jetzt wie die Referenz: hohl-weisses Band mit
  schwarzem Ring ohne Stipple, Closer mit Rand/Fase/weisser Flaeche.
  Verbleibend (A95, nicht dieses Ticket): der `DISABLED`-Overlay auf dem
  ungetoppten Namen (Referenz gerastert, wir massiv), Fasenbreite,
  Glyphenzentrierung.

## Beobachtungen ausserhalb des Umfangs (unveraendert)

* `wind_open` modelliert fuer ein Fenster mit **nicht-nullem** altem
  `WS_CURR` (nach `wind_set(WF_CXYWH)` auf ein erzeugtes, ungeoeffnetes
  Fenster) nicht die Top-Request-/Size-/Move-Zweige von `draw_change`
  (`:1108-1191`); der neue Block ist wie die Quelle auf `WS_PREV == gl_rzero`
  gegated und greift dort nicht — vorbestehende Vereinfachung, eigener Task.
* `compose` malt im Oeffnen-Fall den Hintergrund unter dem **Arbeitsbereich**
  des neuen Fensters nicht (Desktop-Stuecke sind dort leer) — wie EmuTOS,
  das den Bereich der `WM_REDRAW` ueberlaesst; alte Pixel des vorher dort
  liegenden Fensters bleiben bis dahin sichtbar (beidseitig gleich).

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht). Kein Commit.
