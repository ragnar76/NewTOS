# A24 — Quittung

**Status: DONE** · 2026-09-12 · Messluecke „verschachtelte FIRST/NEXT ueber zwei
Fenster" geschlossen — **zugunsten von NewTOS**

## Was der Auftrag war

Eine Messluecke, kein gemeldeter Defekt: laeuft der Rechtecklisten-Zeiger pro
Fenster oder global? Bei einem globalen Zeiger wuerde ein `WF_FIRSTXYWH` auf
Fenster B den laufenden Durchgang auf Fenster A zerstoeren.

## Wie sie geschlossen wurde

Nicht durch Ableitung, sondern gemessen. Falltabelle
`tosdump/cases/aes-r3-wind-state.tsv` (39 Faelle, CRC `979FAC60`), gebaut am
2026-09-11 und am 2026-09-12 erstmals gegen EmuTOS gefahren:

* **EmuTOS-Referenz:** `tosdump/reference/EMUTOS14-AES-r3.BIN`, erzeugt ueber
  Hatari mit `etos512de.img` (bitgleich mit dem ROM der TT-Karte), TT-Medium
  640x480/4 Planes. Das Verfahren ist **bitgleich gegen zwei echte TT-Dumps
  validiert** (`aes-r2b-rlist` 15 Faelle, `aes-r2-wind` 78 Faelle).
* **NewTOS:** `measurement-2026-09-11-qemu-r3/` (QEMU `raspi4b`, dasselbe
  640x480-Profil) und `measurement-2026-09-11-pi4-r3/` (echte Pi 4).

## Ergebnis

**Fall 253 stimmt: der Zeiger ist pro Fenster.** Der verschachtelte Durchgang
ueber zwei Fenster stoert sich nicht. NewTOS ist hier richtig; es gibt nichts zu
reparieren.

Die Faelle 261 und 280 derselben Gruppe weichen zwar ab, sind aber **kein Soll**:
EmuTOS gibt dort den Stapelrest seines eigenen Dispatchers zurueck (`xif`s
uninitialisiertes lokales `int_out[]`, `aes/gemsuper.c:430-432,444-445`).
Nachgewiesen, nicht vermutet: mit einem anderen Zwischenaufruf folgen die Werte
**dem Zwischenaufruf** statt dem Fensterzustand.

## Beleg

`docs/analysis/aes-r3-wind-state-einordnung-2026-09-12.md` (Befund B, Abschnitt
Tickets). Der Auditor hat dafuer eine eigene 16-Fall-Probetabelle gebaut und in
Hatari gegen EmuTOS gefahren.

## Offen

Nichts an A24. Der Bericht benennt getrennt davon zwei echte Defekte an
benachbarten Pfaden (`wind_create` schreibt `WS_WORK`; Zerlegungsreihenfolge
Handle statt z) — die haben eigene Tickets.
