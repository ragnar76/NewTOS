# DESK21 — `Set background` steht im falschen Menue

Status: **OFFEN** · Prioritaet: **P4** · Typ: Konformitaet — Nebenbefund DESK05

Abhaengigkeiten: —

## Sachverhalt

`BACKGRND` ist in EmuTOS Objekt **46** und damit das **letzte** Element des
**View**-Menues, hinter `SEP_VW2` (45); das Options-Menue beginnt erst bei
`IICNITEM` (48) — `references/emutos/desk/desk_rsc.h:35-48`.

NewTOS zeigt `Set background...` als **erstes** Element von *Options*
(`kernel/src/desk/menu.rs`, sichtbar in
`measurement-2026-09-23-desk05/screendumps/conf-C-options.png`).

Betrifft nur die Einordnung im Menue, nicht die Kuerzelzuordnung (die ist in
`shortcut_mapping` semantisch, nicht nach Position).

## Arbeit

Den Punkt ans Ende des View-Menues verschieben, Trennstrich davor wie im
Baum; `shortcut_mapping` und `men_update` entsprechend mitziehen. Screendump
beider Menues vor/nach.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
