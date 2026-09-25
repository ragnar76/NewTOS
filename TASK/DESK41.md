# DESK41 — `TERADESK.INF` gehoert ins Kartenabbild, nicht auf die Karte

Status: **OFFEN** · Prioritaet: **P2** · Typ: Werkzeugkette + Ergaenzung — Pi-Runde 2026-09-25

Abhaengigkeiten: —

## Was passiert ist

TeraDesk kannte nach dem Kartenaufraeumen unsere Dateiendungen nicht mehr.
Ursache: **der Lead hat `TERADESK.INF` geloescht** — sie sah wie eine alte
Desktop-Variante aus, war aber TeraDesks Konfiguration.

Die Datei registriert die Endungen in `ptype`-Bloecken:

```
	ptype=
	{
		name=*.NAP
		flag=0x2001
	}
```

Belegt: `*.NAP` und `*.NCP` stehen darin
(`/Volumes/Daten/Development/newtos-card-archive-2026-09-24/C-BOOT/TERADESK.INF`,
322 Zeilen). Eine Kopie liegt in `hw-round-2026-09-24/TERADESK.INF`.

## Zwei Befunde, nicht einer

1. **Sie war handgepflegt und lag nur auf der Karte.** Damit ist sie bei
   jedem Neubespielen weg — der Fehler war unvermeidlich, nur der Zeitpunkt
   war Zufall. `tools/make-sd-image.sh` muss sie **erzeugen**, wie es
   `NEWDESK.INF`s Inhalt und die Programme erzeugt. Solange sie nicht im
   Abbildbau steht, ist sie nicht reproduzierbar.
2. **Sie kennt nur `.NAP` und `.NCP`.** `.NTP` (nimmt eine Kommandozeile) und
   `.NAC` (Accessory) fehlen — die gab es zum Zeitpunkt der letzten
   Handpflege noch nicht. ADR-018 fuehrt inzwischen alle vier.

## Arbeit

1. Die Erzeugung nach `tools/make-sd-image.sh` ziehen, mit allen **vier**
   Endungen der ADR-018-Familie. Vorher an TeraDesks Quelltext
   (`sdk/apps/teradesk`) klaeren, was die `flag`-Werte bedeuten und welcher
   fuer `.NTP` und `.NAC` richtig ist — `0x2001` fuer `.NAP` nicht blind
   uebernehmen.
2. Klaeren, ob TeraDesk die Datei im Wurzelverzeichnis sucht oder neben
   seinem Programm; die Karte hatte sie in der Wurzel.

Abnahme: frisches Abbild bauen, TeraDesk starten, ein `.NAP`, ein `.NCP` und
ein `.NTP` zeigen ihr Programmsymbol und starten per Doppelklick.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
