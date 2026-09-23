# DESK09 — Kontextspeicher: `cnx_put`/`cnx_get` (Fenster und Einstellungen ueberleben)

Status: **OFFEN** · Prioritaet: **P2** · Typ: Strukturluecke desk/ (G2 der Lueckenliste)

Abhaengigkeiten: —

## Sachverhalt

`docs/analysis/desk-aes-gaps-2026-09-14.md` § G2: es gibt keinen
Kontextspeicher. EmuTOS `cnx_put` (`desk/deskmain.c:1365+`) sichert
Fensterpositionen, Anzeigeart, Sortierung und Einstellungen in den
`.INF`-Satz; `cnx_get` liest sie beim Start zurueck. Bei uns ueberlebt
nichts davon einen Neustart — `Save desktop` schreibt nur einen Teil.

## Arbeit

`cnx_put`/`cnx_get` 1:1 nach der Quelle (welche Felder, welche
`#W`/`#E`-Zeilen), Anbindung an `Save desktop` und an den Startpfad
(`inf.rs`); Hosttest ueber einen `.INF`-Rundlauf, QEMU: Fenster oeffnen,
sichern, neu starten, Fenster steht wieder.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
