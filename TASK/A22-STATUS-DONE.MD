# A22 — Quittung

**Status: DONE** · 2026-09-12 · Messluecke „drei und mehr ueberlappende
Fenster" geschlossen, Defekt belegt, **repariert und gegengemessen**.

Diese Datei loest `A22-STATUS-UNFINISHED.MD` ab (gleicher Vorgang, neuer
Stand — die Namenskonvention der uebrigen Tickets fuehrt den Status im
Dateinamen).

## Was gemessen wurde

Falltabelle `tosdump/cases/aes-r3-wind-state.tsv` (39 Faelle, CRC `979FAC60`),
EmuTOS-Referenz `tosdump/reference/EMUTOS14-AES-r3.BIN` (ueber Hatari,
`etos512de.img`, TT-Medium 640x480/4 Planes; Verfahren bitgleich gegen zwei
echte TT-Dumps validiert), NewTOS in `measurement-2026-09-11-qemu-r3/` (vorher)
und `measurement-2026-09-12-qemu-r3-a22/` (nachher) auf demselben Profil.

## Der Befund, und wo er sass

`border_pieces` (`tos-api/src/aes/window.rs:788-792` im alten Stand) lief
`slots().iter()` durch und subtrahierte jedes hoehere Fenster in der
Reihenfolge, in der es im Fenster-Slot-Array steht — also in Handle-/
Slotreihenfolge.

EmuTOS schneidet in **aufsteigendem z**. `draw_change` baut saemtliche Listen
mit einem einzigen `everyobj(gl_wtree, ROOT, NIL, newrect, …)`
(`aes/gemwmlib.c:1089`); `everyobj` laeuft die Geschwisterkette
`ob_head` -> `ob_tail` (`aes/gemobjop.c:76-131`), und diese Kette **ist** die
z-Ordnung, weil `w_obadd` neu geoeffnete Fenster an den `ob_tail` haengt
(`gemwmlib.c:186-201`, aus `wm_open` `:1425`) und `wm_mktop` getoppte per
`ob_order(gl_wtree, w_handle, NIL)` ebenfalls dorthin schiebt (`:1605-1610`,
`aes/gemoblib.c:1213-1240`, Zweig `new_pos == NIL`). `newrect` setzt die eigene
Liste auf den ungeschnittenen `WS_TRUE`-Rect zurueck und laesst dann
`everyobj(tree, ROOT, wh, mkrect, …)` die Listen aller Fenster **unter** `wh`
mit `wh`s Rechteck aufbrechen (`aes/gemwrect.c:165-205`) — ueber den ganzen
Sweep gelesen: die Liste eines Fensters wird einmal je hoeherem Fenster
geschnitten, in aufsteigendem z dieser verdeckenden Fenster.

Alle vier Quellenangaben sind gegen den gepinnten Baum
`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df` nachgelesen.

## Die Reparatur

`border_pieces` sammelt die verdeckenden Fenster erst ein, sortiert sie stabil
nach aufsteigendem `z` (Einfuegesortierung, hoechstens fuenfzehn Elemente; bei
gleichem `z` bleibt die Slotreihenfolge) und subtrahiert sie dann einzeln. Die
Datenstruktur bleibt das feste Array — frei ist laut Spezifikation nur die
interne Darstellung, nicht die Reihenfolge und nicht das Ergebnis.

## Gegenmessung

`measurement-2026-09-12-qemu-r3-a22/` (QEMU `raspi4b`, 640x480, `#Z`-Autostart,
ohne Karte und ohne Klick). Gegen den Lauf vom 11.09. auf derselben Tabelle:

* Weggefallene Feldunterschiede: 528 `int_out[4]`, 549 `int_out[2..4]`,
  561 `int_out[1..4]` — **die drei Faelle fallen zusammen**.
* Neu aufgebrochene Feldunterschiede: **keine**.
* Unveraendert richtig: 431, 439, 448, 458, 469 (Gruppe I), 502, 511 (das
  mittlere Fenster), 519, 538 (Gruppe II).
* Unveraendert und ohne Soll: die sieben P1-Faelle 261, 280, 400, 481, 494,
  574, 588. Das sind EmuTOS' uninitialisierte Dispatcher-Ausgabeworte
  (`aes/gemsuper.c:430-432,444-445`), weder verbessert noch verschlechtert.
* 340/344 gehoeren zum getrennten Ticket A34 (`wind_create` schreibt
  `WS_WORK`) und wurden nicht angefasst.

## Dauertests

`tos-api/tests/aes_window_rectlist_zorder.rs`, fuenf Faelle: beide Gruppen mit
den Stuecken aus dem EmuTOS-Dump, das mittlere Fenster, die Flaechen-/
Disjunktheitsprobe und der `WF_TOP`-Weg. `cargo run -p xtask -- test`
**452 -> 457 gruen / 0 rot**; **kein bestehender Test musste geaendert werden**
— keiner schrieb die Slotreihenfolge fest, weil keiner drei Fenster oeffnete.

## Spezifikation

`docs/api/aes/window-graphics.md`, „`wind_get` (104)" §
„The rectangle-list iteration" -> „The subtraction order is normative —
ascending z, not handle order". Wurde am 2026-09-12 vor dieser Reparatur
korrigiert; der Code folgt ihr jetzt.

## Die zwei offenen Fragen des Auditors

1. **Verliert die Handlereihenfolge Flaeche?** Nein — sie schneidet nur
   anders. Gerechnet an den gemessenen Listen: Gruppe I
   296*17 + 29*162 + 65*80 + 149*15 = 17165 Pixel, Gruppe II
   296*17 + 29*80 + 65*80 + 29*82 + 149*15 = 17165 Pixel. Die beiden Stuecke
   `31,80,29,80` und `31,160,29,82` der Gruppe II sind zusammenhaengend und
   ergeben genau das eine Stueck `31,80,29,162` der Gruppe I. Allgemein folgt
   das daraus, dass beide Reihenfolgen dieselbe Vereinigung abziehen.
   Schweregrad also: die gezeichnete Flaeche ist gleich, die **Folge**, die ein
   Neuzeichner abarbeitet, ist es nicht.
   *Eine Einschraenkung, nicht gemessen:* die Liste ist auf `RCAP` = 16 Stuecke
   begrenzt und ueberzaehlige werden still verworfen, und weil die Reihenfolge
   die Stueckzahl aendert, koennte eine Reihenfolge ueberlaufen, wo die andere
   es nicht tut. Das ist die Kapazitaetsfrage aus A21, nicht dieses Ticket.

2. **Faellt Baumreihenfolge je von aufsteigendem z auseinander?** Aus der
   Quelle nicht: `w_obadd` (Oeffnen) und `ob_order(…, NIL)` aus `wm_mktop`
   (`WF_TOP`) haengen beide an denselben `ob_tail`. Der neue Test
   `topping_a_window_moves_it_in_the_subtraction_order_like_a_later_open`
   prueft genau das — 1,2,3 oeffnen und dann 2 toppen liefert dieselbe Liste
   wie 1,3,2 oeffnen. **Gemessen ist weiterhin nur der Oeffnungsfall:** die
   Falltabelle hat keinen `WF_TOP`-Fall, eine solche Tabelle fehlt. Die
   Testerwartung steht damit auf der Quelle, nicht auf einem Dump — im Test so
   ausgewiesen.

## Beleg

`measurement-2026-09-12-qemu-r3-a22/` (AES.BIN, AES.LOG, BOOT-SERIAL.txt,
DIFF-emutos-vs-newtos-640x480.txt, RUNINFO.TXT),
`docs/analysis/aes-r3-wind-state-einordnung-2026-09-12.md` Befund C.
