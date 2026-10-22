# CON05 — Absturzbericht sendet `ESC f ESC v` auch auf die serielle Schnittstelle

Status: **OFFEN** · Prioritaet: **P3** · Typ: Nebenbefund CON04-Abnahme (2026-10-22)

## Befund

Seit CON04 beginnt der Absturzbericht mit EmuTOS' `ESC f ESC v`
(`kprint.c:312`). Die Folge geht auch auf die UART; ein einfaches serielles
Terminal zeigt davor „fv“ (`measurements/measurement-2026-10-22-con04-test/RUNINFO.TXT`).

## Auftrag

1. Pruefen, wohin EmuTOS die Folge im Panic-Pfad schickt (nur Bildschirm-Handle
   oder auch die serielle Ausgabe, `bios/kprint.c`).
2. Danach ausrichten: die Folge nur auf den Konsolenpfad, der UART-Spiegel
   bekommt sie nicht — sofern EmuTOS es so haelt.

[Gemeinsame Regeln](README.md) gelten.
