# DESK02 — Options → Set preferences: Einstellungsdialog

Status: **OFFEN** · Prioritaet: **P2** · Typ: Funktionsluecke desk/

Abhaengigkeiten: [DESK08](DESK08.md), [DESK09](DESK09.md) (`cnx_put`/`cnx_get` speichert die Werte)

## Sachverhalt

`Item::SetPrefs` ohne Handler. EmuTOS: `do_optnmenu`/`PREFITEM` →
`inf_pref` (`references/emutos/desk/deskins.c`/`deskinf.c`, Baum `ADSETPRE`):
Doppelklickgeschwindigkeit, Bestaetigung fuer Loeschen/Kopieren/Ueberschreiben,
Zeitformat, Datumsformat, Sortierung. Die Werte landen im `#E`-Satz der
`.INF` (`INF_E1_*`/`INF_E2_*`, `desk/deskapp.c:86-115`).

## Arbeit

Dialog nachbauen, Werte an die vorhandenen Stellen binden
(`ev_dclick`/Doppelklickrate, Bestaetigungsflags in `desk/`), Persistenz
ueber DESK09. Hosttest je Feld, Screendump.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
