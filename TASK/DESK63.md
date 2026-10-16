# DESK63 — Desktop-Verknuepfung auf Programm-Verknuepfung: „illegale Operation“ statt Start

Status: **OFFEN** · Prioritaet: **P3** · Typ: fehlender Zweig — Nebenfund DESK62 (2026-10-16)

`fun_desk2desk` (`programs/desktop/src/dir.rs`, um Zeile 2171) fehlt der Weg
ueber `fun_file2any` → `fun_file2desk`, den EmuTOS geht
(`desk/deskfun.c:1297`, `:1180`). Zieht man ein Datei-Icon vom Desktop auf
eine Programm-Verknuepfung auf dem Desktop, zeigt NewTOS `illegal_op_msg`;
EmuTOS startet das Programm mit der Datei.

Quelle: `docs/analysis/desk62-hourglass-after-drop-on-shortcut-2026-10-16.md`
(Abschnitt „Ausserhalb des Auftrags“). Erst in QEMU reproduzieren und gegen
EmuTOS belegen, dann angleichen; der Merker `exit_desktop` aus DESK62 muss
auch diesen Zweig tragen.

[Gemeinsame Regeln](README.md) gelten.
