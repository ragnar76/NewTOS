# DESK17 — `men_update`: `ntrash`-Regel fehlt; Menuetitel bleibt invertiert

Status: **OFFEN** · Prioritaet: **P4** · Typ: Konformitaet — Nebenbefunde DESK03

Abhaengigkeiten: [DESK03](DESK03.md) (erledigt)

## Die Stellen

1. **`ntrash` fehlt.** EmuTOS zaehlt in `men_update` neben `ndesk` auch die
   ausgewaehlten Papierkorb- und Druckersymbole und schaltet *Delete* ab,
   sobald eines davon in der Auswahl ist
   (`references/emutos/desk/deskmain.c:533-535`). `kernel/src/desk/menu.rs`
   hat seit DESK03 `ndesk`, aber nicht `ntrash` — *Delete* bleibt schwarz.
2. **Der Menuetitel bleibt invertiert, solange ein Dialog offen ist**
   (sichtbar in `measurement-2026-09-24-desk03-desk04/screendumps/ico-B-dialog.png`).
   Verhalten von `hndl_menu`, aelter als DESK03: EmuTOS gibt den Titel frei,
   bevor der Dialog laeuft (`menu_tnormal` in `deskmain.c`s `hndl_menu`,
   gegen die NewTOS-Fassung zu pruefen).

## Arbeit

1 ist eine Zaehlung neben der vorhandenen und gehoert in denselben Durchlauf
ueber die Auswahl. 2 zuerst gegen `hndl_menu` belegen (wo genau gibt EmuTOS
den Titel frei), dann nachziehen; ein Screendump vor/nach mit offenem Dialog
ist die Abnahme.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
