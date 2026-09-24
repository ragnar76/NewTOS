# GUI01 — `CONF_WITH_SHOW_FILE`: Datei per Doppelklick anzeigen

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke AES/Desktop — aus der Schaltertabelle (REF01)

Abhaengigkeiten: —

## Sachverhalt

`CONF_WITH_SHOW_FILE` ist im Referenzbau **1**
(`docs/api/_emutos-config.md`): EmuDesk zeigt den Inhalt einer Datei, die
keiner Anwendung zugeordnet ist, in einer eingebauten Ansicht — Doppelklick
genuegt. Beschreibung des Schalters: `references/emutos/include/config.h:1838-1844`.

In `kernel/src/desk/` findet sich dazu nichts (geprueft 2026-09-24). Ein
Doppelklick auf eine gewoehnliche Datei ist bei uns ein No-op.

## Arbeit

Die EmuTOS-Stelle lesen (Aufrufer in `desk/deskmain.c`/`desk/desksupp.c`,
die Ansicht selbst), dann nachbauen. Die Ansicht haengt mit
[GUI05](GUI05.md) (`CONF_WITH_VIEWER_SUPPORT`) zusammen: dort geht es um
einen **externen** Betrachter, hier um die **eingebaute** Anzeige — erst an
der Quelle klaeren, welche von beiden EmuTOS zuerst versucht.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
