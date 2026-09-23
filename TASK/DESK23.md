# DESK23 — Zwei Fensterrahmen-Eckpunkte bleiben bei `wind_open` ungeschrieben

Status: **OFFEN** · Prioritaet: **P4** · Typ: Defekt — Nebenbefund DESK06

Abhaengigkeiten: —

## Sachverhalt

Ein Fenster, das `wind_open` inkrementell zeichnet, und dasselbe Fenster nach
einer Ganzschirm-Neuzeichnung unterscheiden sich in genau **zwei** Punkten:
(593, 43) und (48, 444) bei 640x480 — die beiden Eckpunkte des Rahmens,
inkrementell **gruen** (Hintergrund), nach der Neuzeichnung **weiss**.

Das ist gemessen und gegen einen Kontrolllauf abgegrenzt: derselbe
Unterschied tritt ueber den `.NCP`-Startpfad auf, der nur dieselbe
`redraw::redraw_desktop` ruft, also gehoert er der Rahmenzeichnung und nicht
dem Programmstart (`measurement-2026-09-23-desk06/`, Laeufe `wdef` und
`redraw`).

Zu klaeren ist, welche der beiden Zeichnungen recht hat — EmuTOS' `w_draw`/
`draw_frame` gegen unsere — und die andere danach zu richten.

## Arbeit

Erst die Quelle: welche Pixel schreibt EmuTOS' Rahmenzeichnung in den zwei
Ecken. Dann die falsche Seite angleichen; zwei Screendumps (inkrementell und
neu gezeichnet) muessen danach pixelgleich sein.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
