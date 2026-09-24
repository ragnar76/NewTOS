# DESK38 — `Show info` gibt es auf einem Desktop-Symbol nicht

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke — Nebenbefund DESK30

Abhaengigkeiten: [DESK30](DESK30.md) (erledigt)

## Sachverhalt

`main::do_info_rename` kehrt sofort um, wenn `G.g_cwin == DESKWH` — auf der
Schreibtischflaeche gibt es also kein *Show info*. Betroffen sind
Laufwerkssymbole, Papierkorb, Drucker und seit DESK30 auch Verknuepfungen.

EmuTOS hat dafuer `do_info` (`references/emutos/desk/desksupp.c:1300-1330`)
mit eigenen Kaesten je Symboltyp — ein Laufwerk zeigt Belegung, der
Papierkorb seinen eigenen Hinweis (`trash_alert`, `:1165-1172`).

Die Luecke ist aelter als DESK30; sie faellt nur jetzt staerker auf, weil es
mehr Symbole gibt, auf die man sie anwenden moechte.

## Arbeit

`do_info` nachziehen, Fall fuer Fall nach der Quelle. Abnahme: *Show info* auf
Laufwerk, Papierkorb, Drucker und einer Verknuepfung, je ein Screendump.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
