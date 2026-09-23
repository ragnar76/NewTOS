# DESK03 — Options → Install icon / Remove icon

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke desk/

Abhaengigkeiten: [DESK08](DESK08.md), [DESK09](DESK09.md)

## Sachverhalt

`Item::InstallIcon` und `Item::RemoveIcon` ohne Handler. EmuTOS:
`ins_devices`/`ins_icon` (`references/emutos/desk/deskins.c`, Baeume
`ADINSDSK`/`ADINSAPP`): ein Symbol auf dem Desktop einrichten (Laufwerk,
Ordner, Papierkorb, Drucker), Typ und Beschriftung waehlen; Entfernen
loescht den `#M`/`#T`-Eintrag. Persistenz in der `.INF`.

## Arbeit

Beide Dialoge, Bindung an unsere Desktop-Symboltabelle
(`kernel/src/desk/global.rs`, `icon.rs`), Persistenz ueber DESK09.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
