# DESK28 — `\AUTO\` setzt kein Arbeitsverzeichnis

Status: **OFFEN** · Prioritaet: **P2** · Typ: Kompatibilitaetsdefekt — Nebenbefund DESK18

Abhaengigkeiten: [DESK18](DESK18.md) (erledigt)

## Sachverhalt

DESK18 hat den Verzeichnisschritt fuer jeden Start **aus dem Desktop**
gebaut. Der zweite Startweg hat ihn nicht: `crate::launch::run_auto_folder`
(`kernel/src/launch.rs`) ruft den Lader direkt, ohne `Dsetdrv`/`Dsetpath`.
Ein `.NCP` in `C:\AUTO\`, das eine Datei mit blossem Namen oeffnet, sieht
deshalb die Wurzel.

Das ist derselbe Defekt wie DESK18 an der anderen Stelle, und es ist der
Weg, ueber den auf einer echten Karte die Treiber und Hilfsprogramme
starten.

## Arbeit

Den Verzeichnisschritt aus `desk::pro::set_default_path` an den
AUTO-Startpfad anlegen — EmuTOS' Ort ist `bios/bios.c`s `autoexec`, dort
gegenpruefen, welchen Pfad TOS dort setzt (Wurzel oder `\AUTO\`). Abnahme:
`CURDIR.NCP` (aus DESK18, `sdk/examples/curdir.c`) nach `C:\AUTO\` legen und
sein Protokoll lesen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
