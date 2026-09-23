# DESK04 — Options → Install devices: Laufwerkssymbole anlegen

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke desk/

Abhaengigkeiten: [DESK03](DESK03.md)

## Sachverhalt

`Item::InstallDevices` ohne Handler. EmuTOS legt fuer jedes vorhandene
Laufwerk der Laufwerkskarte ein Symbol an (`ins_devices`,
`references/emutos/desk/deskins.c`; `#M`-Zeilen wie in
`desk/deskapp.c:715-730`), bestehende bleiben. Bei uns kommen die Symbole
heute nur aus der `.INF` bzw. der Vorgabe.

## Arbeit

Handler nach `ins_devices`; Laufwerkskarte ueber unser BIOS
(`Drvmap`/`bios::drive`), Symbole in die Desktop-Tabelle, Persistenz.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
