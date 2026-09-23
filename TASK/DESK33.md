# DESK33 — `process_funkey`: F1…F20 starten die gebundene Anwendung nicht

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke — Nebenbefund DESK20

Abhaengigkeiten: [DESK20](DESK20.md) (erledigt)

## Sachverhalt

Seit DESK20 kann eine installierte Anwendung eine Funktionstaste tragen
(`APFUNKEY`, angezeigt in *Desktop configuration*). Ein Druck auf die Taste
startet sie aber nicht: `process_funkey`
(`references/emutos/desk/deskmain.c:1087-1106`) hat kein Gegenstueck in
`kernel/src/desk/main.rs`.

Vor DESK20 war das nicht beobachtbar, weil keine Anwendung eine Taste tragen
konnte.

## Arbeit

`process_funkey` nachbauen (Tastencode → `a_funkey` → `do_aopen`), an
EmuTOS' Stelle im Tastaturweg. Abnahme: eine Anwendung auf `F7` legen, `F7`
druecken, sie startet — mit dem Arbeitsverzeichnis aus DESK18.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
