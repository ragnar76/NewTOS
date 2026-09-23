# DESK06 — File → Execute EmuCON: Konsole aus dem Desktop starten

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke desk/

Abhaengigkeiten: —

## Sachverhalt

`Item::ExecEmuCon` ohne Handler. EmuTOS startet seine eingebaute Konsole
(`deskmain.c`, `CLIITEM` → `run_cli`/`pgm_exec`). NewTOS hat eine eigene
Konsole (`kernel/src/shell.rs`, EmuCON-artig, heute nur auf qemu-virt und
ueber die Serielle erreichbar).

## Arbeit

Quelle lesen (wie EmuTOS die Konsole startet und zurueckkehrt), unsere
Konsole aus dem Desktop heraus starten — im Grafikmodus ueber ein
Textfenster oder durch Umschalten nach `sh_toalpha` und zurueck
(`gemshlib.c`); Rueckkehr zum Desktop wie nach einem Programmlauf.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
