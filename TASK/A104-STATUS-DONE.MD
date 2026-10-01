# A104 — STATUS: DONE (2026-09-23) — Fix im Top-Request-Zweig, Rest dokumentiert

**DONE bedeutet hier: gemessen, entschieden, ein Fix gelandet (rot → gruen),
vier Zweige mit Quellenbeleg als Teilmenge festgehalten.** Bearbeitet
2026-09-23 zusammen mit A110 und A91 (ein Paket, ein Messverzeichnis), Branch
`desk-rebuild`, Basis HEAD `671b362`, nichts committet, nichts geflasht, kein
`git checkout/restore/stash/reset`.

## Auftrag

`wind_open` eines Fensters, das per `wind_set(WF_CXYWH)` schon ein Rechteck
hat, laeuft in EmuTOS nicht durch den Oeffnen-Zweig von `draw_change`.
Hosttest, Vergleich der Update-Rechtecke und des Repaints gegen die Quelle —
**Fix nur bei belegter App-Wirkung**, sonst Spec-Note.

## Was der Test gemessen hat

`wind_set(WF_CXYWH, r1)` auf ein erzeugtes, ungeoeffnetes Fenster speichert
`WS_PREV := WS_CURR`, `WS_CURR := r1`, `WS_WORK` und kehrt an der
`VF_ISOPEN`-Pruefung um, ohne zu zeichnen
(`references/emutos/aes/gemwmlib.c:1069-1086`). Das folgende `wind_open(r2)`
betritt `draw_change` also mit `c = r1 != gl_rzero`, und der Baum bei
`:1104-1191` teilt nach dem Verhaeltnis von `r2` zu `r1`:

| `r2` vs. `r1` | Zweig | EmuTOS | Unterschied zu uns |
|---|---|---|---|
| gleich | Top-Request `:1110-1136` | `w_cpwalk(oldtop, 0, MAX_DEPTH, TRUE)` (`:1118-1123`); bei `clrold && wasclr` danach `w_cpwalk(gl_wtop, …)` + `return` (`:1129-1135`) | **Bild** |
| gleicher Ursprung, andere Groesse | Size-Request `:1138-1154` | Schaden = Vereinigung beider Groessen + Schatten; beim Schrumpfen `stop = w_handle`, `moved`, `start = DESKWH` | nur Ausdehnung |
| Obermenge, anderer Ursprung | Grow/Open `:1160-1167` | `c = pt` | **keiner** |
| gleiche Groesse, anderer Ursprung | `w_move` `:1173-1177` | blittet den Schirm bei `r1` nach `r2`, `moved`, kein `WM_REDRAW` | **Bild** |
| sonst | `:1184-1190` | Schaden = Vereinigung von `r1` und `r2`, `start = DESKWH` | nur Ausdehnung |

Zwei Bildunterschiede, drei Ausdehnungsunterschiede — die Bedingung des
Tickets fuer "nur Spec-Note" (Unterschied auf die Ausdehnung beschraenkt, gleiche
Pixel) ist also **nicht** flaechendeckend erfuellt. Deshalb ein Fix, aber ein
begrenzter.

### Warum genau ein Zweig gefixt wurde

* **Top-Request.** Der Aufruf `w_cpwalk(oldtop, …)` bei `:1120` ist **derselbe
  vollstaendige ungetoppte Repaint des vorherigen Top-Fensters**, den A94
  verdrahtet hat — nur haengt A94s Aufruf am Gatter `:1203-1206`
  (`WS_PREV(gl_wtop) == gl_rzero`), das bei einem vorgesetzten Rechteck
  scheitert. Der Zweig ist auch nicht das fruehe `return` bei `:1113-1116`: das
  Fenster ist durch `w_obadd` (`wm_open`, `:1425`) gerade `ob_tail` geworden und
  kann nicht `oldtop` sein, weil es nicht offen war. Die Wirkung ist sichtbar
  und von derselben Klasse, die A94 abgestellt hat (stehengebliebene getoppte
  `IP_2PATT`-Tinte im Namensband des alten Tops).
* **`w_move`-Blit.** Hier wuerde Treue bedeuten, Bildschirminhalt fortzutragen,
  auf dem das Fenster **nie stand** — fuer einen *move* sind das die eigenen
  Pixel des Fensters, fuer ein *open* ist es der Desktop oder ein fremdes
  Fenster. Als benannte Abweichung festgehalten statt nachgebaut.
* **Ausdehnung (Size/Shrink/`:1184-1190`).** Der Zusatzboden ist genau das
  Rechteck, an dem das Fenster nie gezeigt wurde; ihn neu zu malen schreibt die
  Pixel zurueck, die schon dastehen. Mit einem eigenen Testfall belegt.
* **`clrold && wasclr`-Rueckkehr.** EmuTOS laesst dort `w_update` aus, das
  Fenster bekommt also **nie** ein `WM_REDRAW` und zeichnet seinen Inhalt nie.
  Wir posten es wie bei jedem anderen Oeffnen; benannte Abweichung.

## Was gebaut ist

* **`tos-api/src/aes/window.rs`** `wind_open`: der A94-Block bekommt den zweiten
  Term. `let top_request = r != Grect::default() && r == old_curr;` und das
  Gatter lautet jetzt `old_top != DESKWH && old_top != handle && (old_curr ==
  Grect::default() || top_request)`. Kommentar mit dem C-Auszug von
  `:1118-1123` und dem Verweis auf die Spec-Tabelle. **Der Normalpfad
  (`old_curr == gl_rzero`) ist unveraendert** — der neue Term kann dort nicht
  greifen.
* **`tos-api/tests/aes_window_open_oldtop.rs`** (A94s Datei, dieselbe
  Rig-Infrastruktur): drei neue Helfer (`create_window`, `set_cxywh`,
  `open_at`), ein Kopfabschnitt mit der Zweigtabelle und **vier** Faelle:
  * `a104_open_at_the_preset_rect_repaints_the_old_top_untopped` (der Fix),
  * `a104_open_at_a_superset_of_the_preset_rect_leaves_the_old_top_alone`
    (Wache: im Grow-Zweig **darf** das alte Top getoppt stehenbleiben —
    `:1204-1206` scheitert dort auch in EmuTOS),
  * `a104_a_preset_the_window_never_showed_at_repaints_to_the_same_pixels`
    (Size- und Diagonalzweig: Schirm bildgleich mit dem schlichten Oeffnen),
  * `a104_open_at_a_same_size_move_of_the_preset_paints_the_window_not_a_blit`
    (die Quittung fuer die benannte Blit-Abweichung).
* **`docs/api/aes/window-graphics.md`** § `wind_open`: neuer Abschnitt "A preset
  `WS_CURR` — which branches of `draw_change` an open can take (A104)" mit der
  Zweigtabelle, was modelliert ist, warum der Rest nur Ausdehnung ist, und den
  zwei benannten Abweichungen.

## Tests: rot vor, gruen nach

`measurements/measurement-2026-09-23-a104-a110/host-tests/a104-RED-before-fix.txt` /
`a104-GREEN-after-fix.txt`:

| Test | vor | nach |
|---|---|---|
| `a104_open_at_the_preset_rect_repaints_the_old_top_untopped` | „h1 after h2 opened at its preset rect: untopped W_NAME is hollow white outside the glyphs, at (35,43): left 1 (BLACK) right 0 (WHITE)" | gruen |
| die drei uebrigen `a104_*` | gruen (Wachen) | gruen |
| die drei A94-Faelle | gruen | gruen |

`cargo run -p xtask -- test`: **926 passed / 0 failed / 1 ignored**
(Basis 919/0/1; +4 hier, +3 A110).

## Bauen / Clippy

| Beleg | Ergebnis |
|---|---|
| `cargo xtask build --board rpi4` **und** `--board qemu-virt`, je nach `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` | tos-api **64** / kernel **94** auf beiden, rc 0 — Delta 0 |
| `cargo clippy -p tos-api --target aarch64-unknown-none-softfloat` | **80** — unveraendert; `window.rs` 11 → 11 Treffer |

## QEMU

A104 hat **keinen** Aufrufer im Desktop, in TeraDesk oder in QED; ein
Bildschirmbeleg fuer den gefixten Zweig ist headless nicht erzeugbar, ohne ein
Programm zu schreiben, das genau diese Folge ruft. Der Beleg ist der Hosttest
(er rendert durch dieselbe Zeichenkette in einen Pixelpuffer). Die drei
QEMU-Laeufe des Pakets (Kernel-Desktop, A107-Sonde, TeraDesk-Kontrolle) dienen
hier als Nichtregressionsbeleg: der neue Term darf den Normalpfad nicht
anfassen, und tut es nicht — TeraDesk und die Sonde sind **0 px** gegen die
Referenz von A107/A109.

## Offen

Nichts Eigenes. Die zwei benannten Abweichungen stehen in der Spec; der
Blit-Fall waere erst dann wieder aufzumachen, wenn ein Programm diese Folge
wirklich ruft.
