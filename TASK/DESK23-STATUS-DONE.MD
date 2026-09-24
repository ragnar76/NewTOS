# DESK23 — STATUS: DONE (2026-09-24) — Praemisse zurueckgezogen, kein Patch

**Auftrag:** Erst die Quelle: welche Pixel schreibt EmuTOS' Rahmenzeichnung in
den zwei Ecken. Dann die falsche Seite angleichen.

**Ergebnis: keine der beiden Seiten ist falsch.** Beide Zeichnungen sind
EmuTOS-treu, und es sind **vier** Punkte, nicht zwei. Kein Code geaendert.

Basis: Branch `desk-rebuild`, HEAD `3f6b6ef`. Nichts geflasht, nichts
ausgeworfen, keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurement-2026-09-24-desk22-desk23-desk28-desk29/`](../measurement-2026-09-24-desk22-desk23-desk28-desk29/)
§4. Spezifikation: `docs/api/aes/window-graphics.md`, Abschnitt *the drop
shadow's four unwritten pixels (DESK23)*.

---

## 1. Was die Quelle sagt

Das Fenster ist `(48,43,544,400)` — gemessen, nicht angenommen:
`serial/d29-serial.log`, `[aes] op 104 in=(1,5,…) out=(1,48,43,544,400)`
(`wind_get(1, WF_CXYWH)`). `W_BOX` hat Randdicke 1 (`ob_spec 0x00011101`,
`gemwmlib.c:63,84`). `ob_draw` malt

* den Rand mit `gr_box(t.g_x, t.g_y, t.g_w, t.g_h, 1)` → x 48..591, y 43..442;
* die `SHADOWED`-Dekoration **nach** `gr_inside(&t, th)`
  (`aes/gemoblib.c:889-900`):
  `bb_fill(… t.g_x, t.g_y+t.g_h+th, t.g_w+th, 2*th)` → x 49..591, y 443..444,
  `bb_fill(… t.g_x+t.g_w+th, t.g_y, 2*th, t.g_h+3*th)` → x 592..593, y 44..444.

Jedes Band beginnt einen Punkt **innerhalb** seiner Kante. Vier Punkte
gehoeren weder zum Rand noch zu einem Band: **(592,43), (593,43), (48,443),
(48,444)**. `tos-api/src/aes/window.rs`s `rect_outline`/`draw_shadow` rechnen
dieselben Rechtecke aus — die Geometrie ist bereits identisch.

Der Desktop faellt ebenfalls aus: alle vier liegen **innerhalb** des `WS_TRUE`
des Fensters (Rechteck plus `DROP_SHADOW_SIZE`, `gemwmlib.c:253-261`, hier
48..593 x 43..444), die Rechtecklisten entstehen aus `WS_TRUE` (`newrect`,
`gemwrect.c:190`), und `w_drawdesk` laeuft die Liste des **Desktops** ab
(`do_walk(DESKWH, …)`, `gemwmlib.c:340-359`). **Niemand** schreibt diese vier
Punkte.

## 2. Warum die Messung dann zwei Punkte sah

Weil ihre Farbe das ist, was vorher auf dem Schirm stand. Tabelle aus
`method/pixels.py` ueber alle 22 Screendumps dieses Laufs:

| Screendump | (592,43) | (593,43) | (48,443) | (48,444) |
|---|---|---|---|---|
| `d22-*-B-window` (Fenster ueber dem Desktop) | weiss | gruen | weiss | gruen |
| `d22-*-C-program` (Konsole hat geloescht) | weiss | weiss | weiss | weiss |
| `d22-*-D-after` (Desktop neu gezeichnet) | weiss | weiss | weiss | weiss |
| `d29-B` … `d29-F` (nie geloescht) | weiss | gruen | weiss | gruen |

Der Desktophintergrund ist ein Gruen/Weiss-Raster — deshalb tauchten im
DESK06-Diff nur zwei der vier Punkte auf: bei (592,43) und (48,443) ist das
Raster ohnehin weiss und stimmt mit dem Weiss der Konsole ueberein.

`d22-*-C-program` ist der entscheidende Zwischenschritt: **dort sind die
Punkte schon weiss**, bevor die Neuzeichnung ueberhaupt laeuft. Die
Neuzeichnung faerbt sie nicht — sie laesst sie stehen.

## 3. Der Kontrolllauf in die andere Richtung

`d29` oeffnet zwei Fenster, waehlt aus, toppt hin und her — fuenf
Fensteroeffnungen, Toppings und Ganzschirm-Rahmenzeichnungen, kein einziges
Schirmloeschen. Alle vier Punkte bleiben von `d29-A-idle` bis
`d29-F-win2-back` auf dem Hintergrundmuster. Eine Rahmenzeichnung, die die
Punkte „falsch weiss" macht, gibt es also nicht.

## 4. Pruefung

Kein Code geaendert; die Bau- und Testwerte dieses Stapels stehen in der
DESK28-Quittung §4 (64/94 je Board, Clippy 80/125, 938/0/1, `fsck_msdos -n`
sauber, 0 `[EXC]`/`[PANIC]`).

## 5. Offen

Hardwarebestaetigung: Punkt **57** in
`docs/api/desktop/_hw-test-2026-09-17.md`. Ein Befund ist es nur, wenn
**mehr** als diese vier Punkte betroffen sind.

## 6. Was in ein neues Ticket gehoert — nichts

Der Unterschied verschwindet erst, wenn der Rueckweg aus einem
Konsolenprogramm den Schirm so aufbaut, dass auch die Schattenkerbe wieder
Hintergrund zeigt — das waere eine Abweichung von EmuTOS, nicht eine
Annaeherung. Absichtlich kein Folgeticket.
