# GUI06 — Der Boden der Geste: zwei Vollbild-Neuzeichnen und 400–600 MB Cachepflege

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt / Laufzeit — aus der FS04-Messung herausgeloest

Abhaengigkeiten: — (unabhaengig von [FS04](FS04.md); dieselbe Messung, andere Ursache)

## Der Befund, gemessen

Pi 4, **1920x1080**, Sonde aus `measurement-2026-09-25-fs04/`, Bericht
`raw/pi04-FS04.TXT`. Sobald die Karte warm ist, kostet ein Fallenlassen im
Desktop noch **267–340 ms**. Davon:

| | |
|---|---|
| Kopie (GEMDOS gesamt) | 14–87 ms |
| **2x `redraw_desktop`** | **219–234 ms** |

**Rund 85 % der verbleibenden Zeit sind die zwei Vollbild-Neuzeichnen**, je
~110 ms. Das ist der Boden, unter den die Geste nicht kommt, gleich wie warm
die Karte ist. In QEMU bei 640x480 kostete dasselbe 3,13 ms je Aufruf — die
gemessene Skalierung auf 1920x1080 ist x6,19, und der Rest ist die echte
Pixelzahl.

## Die Cachepflege daneben

Je Fallenlassen: `fb.clean` **2 176–3 647 Aufrufe ueber 313–618 MB**,
groesster Einzelaufruf **3 486 720 Byte** fuer ein Primitiv. Der Bildschirm
selbst hat 1920x1080x4 = **8,3 MB**. Es wird also das **40- bis 75-fache** des
Bildschirms durch die Cachepflege geschickt.

Lesbare Ursache: `kernel/src/vdi_backend.rs:284-305` pflegt die
**einschliessende Adressspanne** eines Primitivs statt der beruehrten Zeilen.
Ein schmales, hohes Rechteck bezahlt damit die volle Breite mal Hoehe. In
QEMUs TCG ist `dc cvac` kostenlos, auf einem Cortex-A72 nicht.

## Zwei Fragen, getrennt zu beantworten

1. **Warum zwei Vollbild-Neuzeichnen?** Ein Fallenlassen zeichnet den Desktop
   zweimal komplett neu (`desk.redraw_desk n=2` in **jedem** der 14 Bloecke).
   Gegen EmuTOS zu halten: `deskfun.c`/`deskdir.c` nach dem Kopieren —
   wieviele Neuzeichnen loest EmuTOS aus, und ueber welchen Bereich? Ein
   Vollbild-Neuzeichnen fuer eine geaenderte Verzeichnisliste ist zu pruefen,
   nicht anzunehmen.
2. **Warum die einschliessende Spanne?** `vdi_backend.rs:284-305` gegen die
   tatsaechlich geschriebenen Zeilen. Zeilenweise Pflege ist die naheliegende
   Form; ob sie hier richtig ist, entscheidet die Schreibgeometrie des
   jeweiligen Primitivs.

## Auftrag

Erst messen, welcher der beiden Anteile wieviel der ~230 ms traegt (die Sonde
hat beide Zaehler bereits: `desk.redraw_desk` und `fb.clean`/`fb.clean_lines`),
dann je Anteil die Ursache belegen. Kein Fix vor der Zuordnung.

## Warum P2

Der Oktober-Termin verlangt, dass nichts haengt. Eine Vierteilsekunde je
Mausgeste ist kein Haenger, faellt aber bei jedem Klick auf — und sie liegt
nicht an der Karte, laesst sich also unabhaengig von FS04 beheben.

## Abnahme

Dieselbe Geste, gemessene Zeit vorher/nachher aus derselben Sonde, und die
Zuordnung der Ersparnis auf die beiden Anteile.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
