# DESK15 — Fenstersymbole und Desktop-Verknuepfungen (`ADINSWIN`)

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke — gemeldet in DESK03

Abhaengigkeiten: [DESK03](DESK03.md) (erledigt)

## Sachverhalt

`kernel/src/desk/ins.rs` hat die Form eines `CONF_WITH_WINDOW_ICONS = 0`-Baus:
`ins_icon(0)` geht direkt auf `install_desktop_icon(NULL)`
(`references/emutos/desk/deskins.c:1262-1263`, der `#else`-Zweig), statt mit
`STICNTYP` zu fragen, ob ein Desktop- oder ein Fenstersymbol gemeint ist. Der
Grund ist kein Zeitmangel, sondern eine fehlende Ebene: beide Merkmale haengen
einen `ANODE` an einen **Dateinamen**, und NewTOS' `win_bldview` nimmt das
Symbol eines Fensterobjekts aus der Endung (`ftype::classify`), nie aus einer
Zuordnung. `ADINSWIN` waere ohne diesen Leser ein Bedienelement ohne Wirkung.

Betroffen sind damit auch `CONF_WITH_DESKTOP_SHORTCUTS` (ein Desktop-Symbol,
das auf eine Datei oder einen Ordner zeigt) — dieselbe fehlende Ebene.

## Arbeit

Reihenfolge ist vorgegeben: **zuerst** der Leser. `win_bldview` muss das Symbol
eines Fensterobjekts aus der `ANODE`-Liste beziehen koennen (EmuTOS
`deskwin.c`/`deskapp.c` `app_afind_by_name`), mit der Endungsregel als
Rueckfall. Erst danach `ADINSWIN` (`desk_rsc.c`) und der `STICNTYP`-Zweig in
`ins_icon`.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
