# PORT09 — `PT_ARGV` bei `*.NAP` verspricht ARGV, unser Splitter kennt es nicht

Status: **OFFEN** · Prioritaet: **P3** · Typ: SDK / Startschnittstelle — Nebenfund aus [DESK41](DESK41.md)

Abhaengigkeiten: —

## Sachverhalt

Die von [DESK41](DESK41.md) erzeugte `TERADESK.INF` traegt fuer `*.NAP` das
Flag `0x2001` = `PD_PDIR | PT_ARGV` (`sdk/apps/teradesk/prgtype.h:42,55`).
Das ist **belegter Kartenstand** und zugleich TeraDesks eigene Vorgabe fuer
`.PRG`/`.APP` (`prgtype.c:346-347`), war also die richtige Wahl.

`PT_ARGV` hat aber eine Wirkung: TeraDesk setzt beim Start
`cl.length = 127` (`startprg.c:654-655`) — das ist das ARGV-Signal der
Atari-Konvention, nicht die Laenge des Schwanzes.

**Unsere `sdk/libc/newtos-arch/cmdargs.c:38-46` kennt die `ARGV=`-Erweiterung
nicht** und liest den Schwanz gewoehnlich.

## Warum es trotzdem laeuft

TeraDesk legt den echten Schwanz **zusaetzlich** in `command_tail`, und unser
Splitter bricht am ersten NUL ab. Praktisch harmlos — aber der Weg ist nicht
der, den das Flag beschreibt. Ein Programm, das sich auf die ARGV-Konvention
verlaesst, bekaeme von uns etwas anderes als angekuendigt.

## Arbeit

1. In den Quellen klaeren, was `cl.length == 127` nach der Atari-Konvention
   genau zusagt (Umgebungsvariable `ARGV=`, NUL-getrennte Liste) — EmuTOS
   und/oder die gepinnten Sekundaerquellen, **nicht** aus dem Gedaechtnis.
2. Entscheiden, ob `cmdargs.c` die Konvention lernt (dann kann das Flag
   stehenbleiben und bedeutet, was es sagt) oder ob wir sie ausdruecklich
   nicht fuehren (dann gehoert das **dokumentiert**, nicht stillschweigend).

Kein Stub: solange die Konvention nicht umgesetzt ist, bleibt sie ein
benanntes „nicht vorhanden".

Abnahme: ein Programm mit mehr Argumenten, als in 125 Zeichen passen,
erhaelt sie vollstaendig — oder die Datei sagt, warum das bei uns nicht geht.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
