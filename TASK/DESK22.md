# DESK22 — `.NCP`-Start laesst den Mauszeiger auf dem Textschirm stehen

Status: **OFFEN** · Prioritaet: **P3** · Typ: Defekt — Nebenbefund DESK06

Abhaengigkeiten: [DESK06](DESK06.md) (erledigt)

## Sachverhalt

DESK06 hat den Konsolenweg ueber EmuTOS' Modusumschaltung gebaut:
`sh_toalpha` (`references/emutos/aes/gemshlib.c:246-260`) nimmt den Zeiger
weg (`gsx_mfset`/`ratexit`) und schliesst die Workstation, bevor der
Textschirm kommt. Der **vorhandene** `.NCP`-Startpfad
(`kernel/src/desk/main.rs` `launch_native`) tut das nicht — er ruft nur
`fbcon::clear_and_home`, und der Zeiger bleibt als schwarzer Fleck im
Textschirm stehen.

Beleg: `measurement-2026-09-23-desk06/screendumps/redraw-C-program.png`,
Fleck bei rund (100, 197). Der neue EmuCON-Weg hat das nicht
(`cli-C-console.png` ist zeigerfrei).

## Arbeit

`launch_native` auf denselben Weg legen, den DESK06 gebaut hat
(`sh_toalpha`/`sh_tographic` ueber `shell_loop`), statt den Schirm nur zu
loeschen. Screendump vor/nach.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
