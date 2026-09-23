# DESK32 — Drei Klappmenues sind eine Zeichenzelle zu breit

Status: **OFFEN** · Prioritaet: **P4** · Typ: Kosmetik — Nebenbefund DESK21

Abhaengigkeiten: [DESK21](DESK21.md) (erledigt)

## Sachverhalt

Desk misst 22 Zellen statt 21, File 23 statt 22. Options (30) und View (23,
seit DESK21) stimmen.

Ursache: die Beschriftungen in `kernel/src/desk/menu.rs`s `MENUS` tragen ein
Fuellzeichen mehr als der laengste Text, und `adjust_menu`s `maxlen` zaehlt
es mit. EmuTOS misst die von `copy_menu_items` gepolsterte Zeichenkette
(`references/emutos/desk/deskmain.c:1781`, `:1608-1620`).

## Arbeit

Die Polsterung aus den `MENUS`-Zeichenketten nehmen und sie wie EmuTOS beim
Bauen entstehen lassen; Screendump aller vier Klappmenues vor/nach mit
gemessener Zellenbreite.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
