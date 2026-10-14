# PROC06 — `Pexec` liefert nach Ctrl-C `-32` vorzeichenerweitert statt 65504

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Nebenfund DESK43 U9-0 (2026-10-14)

EmuTOS legt den Exitcode in `xterm(UWORD rc)` zero-extended ab
(`bdos/proc.c:601,616`): ein per Ctrl-C beendetes Kind (`Pterm(-32)`) kommt
beim `Pexec`-Aufrufer als 65504 an. NewTOS liefert dem EL0-Aufrufer -32
(vorzeichenerweitert). Der Shell-Start (PROC05, U9-0b) rechnet schon mit
`pterm::parent_exit_code` richtig; der `Pexec`-Rueckweg zum Programm noch
nicht. Belegen, ob das fuer alle `Pterm`-Werte gilt, dann angleichen
(Spec `docs/api/gemdos/process/`, Hosttest, APITEST-Fall).

[Gemeinsame Regeln](README.md) gelten.
