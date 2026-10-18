# CON03 — Die Kernel-Konsole ist nicht EmuCON: Befehlssatz und Ort

Status: **OFFEN** · Prioritaet: **P3** · Typ: Abweichung von EmuTOS — Befund 2026-10-18 (Frage des Anwenders nach einem Textbetrachter)

## Befund

EmuTOS' EmuCON (`references/emutos/cli/`) hat 21 Befehle (`cmdint.c:139-162`):
`cat/type`, `cd`, `chmod`, `cls/clear`, `cp/copy`, `echo`, `exit`, `help`,
`ls/dir`, `mkdir/md`, `mode`, `more`, `mv/move`, `path`, `pwd`, `ren`,
`rm/del`, `rmdir/rd`, `show`, `version`, `wrap`.

NewTOS' Konsole (`kernel/src/shell.rs:120-131`) kennt `exit/quit`, `help`,
`ver`, `cls`, `pwd`, `cd`, `dir/ls`, `type/cat`, `run` sowie die
NewTOS-eigenen `bench`, `init`. Es fehlen u. a. `more` (seitenweise Anzeige —
der einzige Textbetrachter in EmuCON), `cp`, `mv`, `ren`, `rm`, `mkdir`,
`rmdir`, `chmod`, `path`, `echo`, `mode`, `show`, `wrap`.

Ort: EmuCON ist in EmuTOS ein eigener Prozess (`aes_run_rom_program(coma_start)`,
`aes/gemshlib.c:614-620`, `cli/cmdasm.S`), NewTOS' Konsole laeuft im Kern (EL1).
PROC03 hat dafuer einen EmuCON-Stellvertreter-PD eingefuehrt.

## Auftrag (spaeter)

1. Spec aus `cli/` (Befehle, Parser, Umlenkung `>`/`<`, Hilfe-Texte).
2. Anwender entscheidet: Befehlssatz 1:1 in der Kernel-Konsole nachruesten
   oder EmuCON wie NEWDESK als EL0-Programm (ADR-026-Muster).
3. Bauen, Tests per Injektor.

Bezug: [GUI01](GUI01.md) — Textbetrachter im Desktop bleibt eine angemeldete
Anwendung (Anwender 2026-10-18 bestaetigt).

[Gemeinsame Regeln](README.md) gelten.
