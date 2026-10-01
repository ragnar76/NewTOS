# DESK41 — STATUS: DONE (2026-10-01) — Abnahme aus TeraDesk heraus auf echter Pi 4 bestaetigt

**Auftrag:** `TERADESK.INF` im Abbildbau erzeugen statt sie von Hand auf der
Karte zu pflegen (`TASK/DESK41.md`). Implementiert 2026-09-29; die
Hardwareabnahme stand seit dem Nachtrag vom 2026-09-30 aus, weil die damalige
Runde aus **unserem** Desktop lief und nicht aus TeraDesk.

## Abnahme

Gefahren vom Anwender auf der echten Pi 4, gemeldet 2026-10-01. Auf
ausdrueckliche Rueckfrage bestaetigt: **aus TeraDesk heraus**, `C:` geoeffnet,
alle vier Punkte gesehen:

1. `.NAP`, `.NCP` und `.NTP` tragen ihr Programmsymbol.
2. Doppelklick startet sie.
3. `.NTP` zeigt davor TeraDesks eigenen Kommandozeilen-Dialog (`applik.c:1390`).
4. `.NAC` wird verweigert (`startprg.c:644`).

Damit ist die Abnahme aus `TASK/DESK41.md` (Abschnitt *Arbeit* und Nachtrag
2026-09-30) woertlich erfuellt.

## Was der Beleg ist — und was nicht

Beleg ist die Anwenderbeobachtung am Geraet. Es gibt keinen Messordner, keine
Screendumps und keine notierte Kern-Revision zu diesem Lauf. Die
QEMU-Gegenprobe vom 2026-09-29 (Datei gefunden, gelesen, fehlerfrei geparst)
bleibt der maschinelle Beleg fuer den Abbildbau.

## Offen geblieben, eigene Tickets

- [DESK49](DESK49.md) — TeraDesks `filetypes`-Liste kennt die vier Endungen nicht.
- [PORT09](PORT09.md) — `PT_ARGV` bei `*.NAP`, unser Splitter kennt `ARGV=` nicht.
