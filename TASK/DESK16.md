# DESK16 — `IG_REMOV` (5) hat kein Raster

Status: **OFFEN** · Prioritaet: **P4** · Typ: Ressourcenluecke — gemeldet in DESK03

Abhaengigkeiten: [DESK03](DESK03.md) (erledigt)

## Sachverhalt

Die `IG_*`-Nummern sind **Dateiformat**: `app_parse`, `app_save` und
`desk_inf_data2` schreiben sie woertlich in die `#M`/`#T`/`#O`-Zeilen
(`references/emutos/desk/deskapp.c`). NewTOS hat Raster fuer 0 Festplatte,
1 Diskette, 2 Ordner, 3 Papierkorb, 4 Drucker, 6 Programm, 7 Dokument —
**`IG_REMOV` (5, Wechselmedium) fehlt** in den aus `desk/icon.rsc`
uebernommenen Bitmaps.

Folgen, beide in DESK03 gemeldet und nicht stillschweigend ersetzt: das
Schrittwerk des `Install icon`-Dialogs laeuft ueber sieben Eintraege statt
`0..g_numiblks-1`, und eine `a_aicon`, die nicht aufgeloest werden kann,
faellt auf das typgerechte Bild zurueck (`app::default_icon_kind`), statt
leer zu zeichnen.

## Arbeit

Das Raster aus `references/emutos/desk/icon.rsc` mit demselben Weg
uebernehmen, den die uebrigen sieben genommen haben, `BUILTIN_ICONS` um den
Eintrag erweitern und den Rueckfall in `default_icon_kind` fuer diesen Fall
entfernen. Danach laeuft das Schrittwerk ueber acht Eintraege.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
