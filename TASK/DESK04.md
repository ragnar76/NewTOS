# DESK04 — Options → Install devices: Laufwerkssymbole anlegen

Status: **ERLEDIGT (2026-09-24) — Pi-Sichtabnahme offen** · Prioritaet: **P3** · Typ: Funktionsluecke desk/

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

## Abschluss

**ERLEDIGT (2026-09-24) — Pi-Sichtabnahme offen.** Quittung:
[DESK04-STATUS-DONE.MD](DESK04-STATUS-DONE.MD).

`desk::ins::ins_devices` (`deskins.c:290-333`) mit `install_drive`/
`remove_drive`, aufgerufen aus `do_optnmenu`s `IDSKITEM`-Fall
(`deskmain.c:847-852`). Die Laufwerkskarte kommt als
`dos_sdrv(dos_gdrv())` ueber den GEMDOS-Trap (DESK10/DESK11). Vorhandene
Symbole bleiben unangetastet — Position, Beschriftung und Bild —, fehlende
kommen an der ersten freien Rasterstelle dazu (`ins_posdisk`), und Symbole
von Laufwerken, die es nicht mehr gibt, verschwinden.

Der Punkt konnte vorher **bauartbedingt** nichts tun: `app_blddesk` leitete
die Symbole bei jedem Aufruf aus der Laufwerkskarte ab, jedes vorhandene
Laufwerk hatte also immer ein Symbol. Dass die Symboltabelle jetzt Zustand
ist (`G.g_ahead`), gehoert zu DESK03 und ist dort beschrieben.

**Geprueft:** `measurements/measurement-2026-09-24-desk03-desk04/` — nach `Remove desktop
icon` holt `Install devices` das Symbol zurueck, und das Bild ist hinterher
**pixelgleich** zum Ausgangsbild (pixdiff 0). `desk::ins::selftest()` deckt
die Faelle ab, die QEMU nicht stellen kann: mehrere fehlende Laufwerke in
einem Zug, ein verschwundenes Laufwerk, `IG_HARD` gegen `IG_FLOPPY`, und dass
Papierkorb und Drucker nicht angefasst werden.

**Offen:** Pi 4, HW-Checkliste Punkt **38**, Schritte 0/3/8 — die **vier**
Laufwerke der Karte.
