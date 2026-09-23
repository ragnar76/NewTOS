# DESK25 — `STDELDIS`: die Zusatzfrage beim Loeschen eines Laufwerks fehlt

Status: **OFFEN** · Prioritaet: **P4** · Typ: Funktionsluecke — Nebenbefund DESK13

Abhaengigkeiten: [DESK13](DESK13.md) (erledigt)

## Sachverhalt

EmuTOS stellt beim Loeschen eines ganzen Laufwerks eine **zweite** Frage
(`STDELDIS`, `references/emutos/desk/deskdir.c:971-975`); sie haengt an
`icontype == AT_ISDISK`.

NewTOS' `dir_op` bekommt den Symboltyp nicht uebergeben, also kann die Frage
dort nicht entstehen. Der Bestaetigungskasten aus DESK13 ist davon
unberuehrt — es geht um die eine zusaetzliche Rueckfrage.

## Arbeit

Den Symboltyp bis `dir_op` durchreichen (EmuTOS' Parameter), dann die Frage
an EmuTOS' Stelle stellen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
