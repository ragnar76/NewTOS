# MEM02 — `Mxalloc` mit `MX_PREFTTRAM` liefert ST-RAM

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt — Pi-Lauf 2026-09-28

Abhaengigkeiten: — · **Vorbedingung fuer [PORT06](PORT06.md) §3** (Dooms Speicherentwurf)

## Der Befund, auf echter Pi 4 gemessen

Die konforme Ball-Demo (Kern `09f3e928`, 1920x1080), Protokoll
`measurement-2026-09-28-port07/raw/pi01-BALL.LOG`:

```
BALL: ST-RAM top 0x00E00000, ramtop 0x2FE00000; buffers asked for alternate RAM (MX_PREFTTRAM)
BALL: buffers idx=2025K/ST bg=2025K/ST bgrow=1K/ST map_off=182K/ST map_shd=91K/ST
      run0=0K/ST run1=0K/ST tex=64K/ST log=32K/ST
```

**Jede einzelne Anforderung ging mit `MX_PREFTTRAM` hinaus und kam als ST-RAM
zurueck** — zusammen rund **4,2 MB aus einem 10-MB-Pool**, waehrend oberhalb
`0x00E00000` bis `ramtop = 0x2FE00000` der grosse Bereich liegt (FS04 mass die
Pools mit **ST-RAM 10 MiB / TT-RAM 942 MiB**).

Die Demo **laeuft** damit — es ist kein Absturz, sondern eine nicht bediente
Praeferenz.

## Warum es mehr ist als eine Unsauberkeit

**Der Doom-Entwurf steht darauf.** [PORT06](PORT06.md) §3 haelt die Richtung
des Anwenders fest: vorrendern und im TT-RAM ablegen, weil die TPA 4 MiB je
Prozess ist und im ST-RAM nur 10 MB liegen. Bedient `Mxalloc` die Praeferenz
nicht, laeuft dieser Entwurf gegen eine 10-MB-Wand — und zwar still, weil der
Aufrufer einen gueltigen Zeiger bekommt.

Dasselbe gilt fuer jede kuenftige Anwendung, die viel Speicher will: ein
Bildbetrachter, ein Emulator, ein SDL-Programm.

## Was zu klaeren ist — ungemessen, nicht vermutet

1. **Wird die Praeferenz ueberhaupt ausgewertet?** `Mxalloc`s `mode`-Argument
   (`MX_STRAM` 0, `MX_TTRAM` 1, `MX_PREFSTRAM` 2, `MX_PREFTTRAM` 3) gegen
   unsere Umsetzung halten — und gegen EmuTOS' `bdos/umem.c`, das die
   Unterscheidung in `MD`-Listen je Pool fuehrt.
2. **Gibt es den TT-RAM-Pool zur Allokation ueberhaupt?** FS04 hat ihn im
   Startprotokoll gesehen (`[mm] BDOS pools: ST-RAM … TT-RAM …`) — ob er an
   `Mxalloc` angeschlossen ist, ist damit **nicht** gesagt.
3. **Kommt der Block im EL0-Raum an?** Ein TT-RAM-Block muss in das
   Adressfenster des Prozesses eingeblendet werden und eine User-VA
   zurueckliefern ([[el0-app-memory-must-be-mapped]]). Ob das fuer den
   TT-RAM-Pfad gebaut ist, gehoert zur selben Frage.

**Die Reihenfolge ist wichtig:** erst feststellen, an welcher der drei Stellen
es aufhoert, dann beheben. Eine Praeferenz zu verdrahten, die unten keinen
Pool hat, waere ein Stub.

## Abnahme

Dieselbe Ball-Demo auf derselben Karte: die Pufferzeilen melden `/TT` statt
`/ST`, und der ST-RAM-Verbrauch faellt entsprechend. Dazu ein Test, der
`Mxalloc` mit allen vier Modi prueft — inklusive `MX_TTRAM` (0x1), das
**erzwingt** und bei fehlendem Pool fehlschlagen **muss** statt still ST-RAM
zu liefern.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
