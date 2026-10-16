# DESK62 — Sanduhr bleibt nach Drop einer Datei auf eine Desktop-Verknuepfung

Status: **OFFEN** · Prioritaet: **P2** · Typ: Fehler — Befund des Anwenders auf echter Pi 4 (2026-10-16, Abnahme DESK43 U9-6)

## Befund

1. Verknuepfung von `QED.NAP` auf dem Desktop anlegen.
2. Eine Textdatei per Drag & Drop auf diese Verknuepfung ziehen.
3. Die Start-Animation laeuft, QED startet. Nach der Rueckkehr zum Desktop
   bleibt der Mauszeiger die **Sanduhr**. Das System laeuft weiter.

Gegenproben des Anwenders:

- Doppelklick auf die Verknuepfung (ohne Datei): funktioniert.
- Datei im Verzeichnisfenster direkt auf `QED.NAP` ziehen: funktioniert.
- Der Fehler bestand schon **vor** DESK43 (eingebauter Desktop), er ist also
  keine Folge des Umzugs nach EL0.

Offen: ob die Sanduhr spaeter von selbst verschwindet (naechster Klick,
Menue, naechster Programmstart); ob QED im Drop-Fall die Datei geladen hat;
serielles Log des Drop-Falls.

## Auftrag

Den Drop-auf-Desktop-Icon-Pfad in EmuTOS lesen (`desk/deskact.c`,
`desk/deskfun.c`, `desk/deskmain.c` — Ziel-Icon ist Desktop-Objekt statt
Fensterobjekt) und mit `programs/desktop/` vergleichen, insbesondere wo
EmuTOS die Maus nach dem Start/der Rueckkehr wieder auf `ARROW` setzt.
Ursache mit Beleg vorlegen (Datei:Zeile beider Seiten) und in QEMU per
Injektor reproduzieren, **bevor** gefixt wird.

[Gemeinsame Regeln](README.md) gelten.
