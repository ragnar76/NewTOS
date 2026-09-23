# DESK19 — Ablegen auf ein Programm startet es nicht (`fun_win2win`)

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke — Nebenbefund DESK05

Abhaengigkeiten: [DESK05](DESK05.md) (erledigt)

## Sachverhalt

In EmuTOS ist das Ablegen einer Auswahl auf ein **Programmsymbol** ein
Programmstart mit Argumenten, kein Kopieren: `fun_win2win`
(`references/emutos/desk/deskfun.c:958-970`) erkennt das Ziel und baut die
Befehlszeile mit `build_selected_path` (`:903-931`), dessen Form die
Desktop-Konfiguration bestimmt (voller Pfad oder nur der Dateiname).

NewTOS kennt diesen Zweig nicht — ein Ablegen auf ein Programm ist eine
Kopie. Deshalb fehlt auch die Radiogruppe *Parameter* (`DCPMFULL`/
`DCPMFILE`) in `Desktop configuration` (DESK05); `INF_E5_ISFULL` wird
gelesen und unveraendert zurueckgeschrieben.

## Arbeit

Den Zweig in `fun_win2win` nachbauen, `build_selected_path` mit, dann die
Radiogruppe in `inf_conf` nachziehen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
