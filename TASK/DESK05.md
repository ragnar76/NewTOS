# DESK05 — Options → Desktop configuration (`.INF`-Einstellungen)

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke desk/

Abhaengigkeiten: [DESK09](DESK09.md)

## Sachverhalt

`Item::DeskConfig` ohne Handler. EmuTOS: der Punkt, der die
Desktop-Einstellungen (Hintergrundmuster/-farbe, Anzeigeart, Sortierung)
in einem Dialog zusammenfasst bzw. `.INF`-Inhalte bearbeitet — genaue
Fundstelle und Umfang aus der Quelle bestimmen (`desk/deskmain.c`
`do_optnmenu`, `desk_rsc.c`-Baeume).

## Arbeit

Erst Quelle lesen und Umfang festlegen (was gehoert hierher, was nach
DESK02), dann bauen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
