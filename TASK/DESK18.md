# DESK18 — Kein Arbeitsverzeichnis beim Programmstart (`set_default_path`)

Status: **OFFEN** · Prioritaet: **P2** · Typ: Kompatibilitaetsdefekt — Nebenbefund DESK05

Abhaengigkeiten: [DESK05](DESK05.md) (erledigt)

## Sachverhalt

EmuTOS setzt vor **jedem** Programmstart das Arbeitsverzeichnis. `do_aopen`
(`references/emutos/desk/desksupp.c:880-915`) waehlt den Pfad nach den
Desktop-Konfigurationsschaltern — installierte Anwendung: ihr eigener Pfad;
sonst `G.g_appdir` (Anwendungsverzeichnis) oder das oberste Fenster — und
ruft `set_default_path(app_path)`; scheitert das, bricht der Start mit
`STDEFDIR` ab.

NewTOS setzt **gar keins**: `desk::main::launch_native` geht ueber
`crate::launch::run_program`, ohne vorher `Dsetpath`/`Dsetdrv` zu stellen.
Ein portiertes Programm, das relativ oeffnet (`Fopen("FOO.TXT")`), sieht
damit irgendein Verzeichnis.

Zweite Folge: die Radiogruppe *Default dir* (`DCDEFAPP`/`DCDEFWIN`) fehlt in
`Desktop configuration` (DESK05), weil sie genau diesen Schalter bedient —
`INF_E5_APPDIR` wird gelesen und unveraendert zurueckgeschrieben, aber von
niemandem ausgewertet.

## Arbeit

`do_aopen`s Pfadwahl nachbauen, mit `dosbind`s Bindungen (`Dsetpath`,
`Dsetdrv`), `STDEFDIR`-Alert im Fehlerfall; danach die Radiogruppe in
`inf_conf` nachziehen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
