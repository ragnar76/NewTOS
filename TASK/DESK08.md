# DESK08 — Dialoge: `form_center` und `form_dial(FMD_START/FMD_FINISH)`

Status: **OFFEN** · Prioritaet: **P2** · Typ: Strukturluecke desk/ (G7 der Lueckenliste)

Abhaengigkeiten: —

## Sachverhalt

`docs/analysis/desk-aes-gaps-2026-09-14.md` § G7: `desk/` zeigt seine
Dialoge ohne `form_center` (Zentrierung) und ohne
`form_dial(FMD_START)`/`FMD_FINISH` (Hintergrund sichern/wiederherstellen,
Wachstums-/Schrumpfkasten). EmuTOS macht beides
(`desk/deskinf.c`, `desk/deskins.c`, jeweils vor `form_do`).

## Arbeit

Die Kette `form_center` → `form_dial(FMD_START)` → `objc_draw` → `form_do`
→ `form_dial(FMD_FINISH)` in den vorhandenen desk/-Dialogen nachziehen
(`dlg.rs`, `inf.rs`, `insapp.rs`); Hosttest: Dialog zentriert, Hintergrund
nach dem Schliessen bytegleich.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
