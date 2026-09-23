# DESK20 — `Install application` vergibt keine Funktionstaste (`APFUNKEY`)

Status: **OFFEN** · Prioritaet: **P4** · Typ: Funktionsluecke — Nebenbefund DESK05

Abhaengigkeiten: [DESK05](DESK05.md) (erledigt)

## Sachverhalt

EmuTOS' `ADINSAPP` hat ein Funktionstastenfeld (`APFUNKEY`,
`references/emutos/desk/desk_rsc.h:119`, Baum `desk_rsc.c:1059`), das
`install_application` liest und schreibt (`deskins.c:410`, `:477`, `:523`) —
so bekommt eine installierte Anwendung ihre Funktionstaste.

NewTOS' `ins_app_dialog` hat das Feld nicht. `a_funkey` wird gefuehrt,
gespeichert und in `Desktop configuration` **angezeigt** (DESK05), aber es
gibt keinen Weg, eine Taste zu vergeben — der Anzeiger bleibt darum leer.

## Arbeit

Das Feld in `ins_app_dialog` nachziehen, samt Lesen/Schreiben wie in
`deskins.c`; danach zeigt der Anzeiger in `inf_conf` etwas.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
