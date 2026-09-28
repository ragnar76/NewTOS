# GUI07 — `mem_to_screen` liest je Pixel den Bildschirm und rechnet um, auch wenn es das Ergebnis wegwirft

Status: **OFFEN** · Prioritaet: **P2** · Typ: Durchsatz — aus [PORT07](PORT07.md), berichtet statt behoben

Abhaengigkeiten: — · **nicht vor dem OFAM-Termin** (kein Kerneleingriff in den Zeichenpfad)

## Warum das jetzt zaehlt

PORT07 hat die Ball-Demo auf den konformen Weg gebracht und dabei den Preis
gemessen: **80,49 ns je Pixel** im `vro_cpyfm`, gegen 4,38 ns auf dem alten
Direktweg. Fuer [PORT06](PORT06.md) (Doom) bedeutet das **5,15 ms je Bild**
allein fuer die Kopie — 18 % eines 35-Hz-Taktes. Jede Anwendung, die ihr Bild
konform auf den Schirm bringt, zahlt diesen Preis.

**Und genau das ist das Argument dafuer, ihn zu senken statt den konformen Weg
zu meiden:** wer ueber die VDI geht, erbt jede Verbesserung — wer daran vorbei
arbeitet, nicht ([PORT06](PORT06.md) §„Das staerkere Argument").

## Die zwei Befunde

**V-a — eine Leseoperation und eine Umrechnung je Pixel, umsonst.**
`tos-api/src/vdi/raster.rs:494-533` (`mem_to_screen`) liest fuer **jedes**
Pixel den lebenden Bildschirmpunkt und rechnet ihn nach RGB565 — auch bei
`S_ONLY` (Modus 3), wo `raster_op(3, s, d) = s` das Ergebnis **wegwirft**.
`S_ONLY` ist der haeufigste Kopiermodus ueberhaupt.

**V-b — zwei virtuelle Aufrufe je Pixel.** Dieselbe Schleife laeuft ueber
`&dyn RasterBackend`: `get_device_pixel`/`put_device_pixel` je Pixel, und
Basisadresse sowie Zeilenabstand werden je Pixel neu aus `active_fb()` geholt
statt einmal je Zeile.

Beides ist **Durchsatz, nicht Verhalten** — kein Bild wird falsch. Deshalb
wurde es berichtet und nicht behoben.

## Der Messstand liegt bereit

Die konforme Ball-Demo **ist** der Pruefstand: sie meldet `ns/px` im Blit und
die freilaufende Bildzeit. Vorher/Nachher ist damit eine Zahl, kein Eindruck.
Verfahren in `measurement-2026-09-28-port07/method/`.

## Auftrag

1. `S_ONLY` (und die anderen Modi, die `d` nicht lesen) **ohne** Bildschirmlesen
   bedienen — welche das sind, steht in der Wahrheitstabelle der 16
   Rasteroperationen und ist abzuleiten, nicht zu raten.
2. Die Schleife auf zeilenweise Arbeit bringen: Basis und Zeilenabstand einmal
   je Zeile, nicht je Pixel.
3. **Kein Verhalten aendern.** Der Beleg ist die Raster-Referenzsuite
   (`tos-api/tests/vdi_raster_reference/`, 59 Faelle gegen echte TT- und
   Falcon-Dumps) — sie muss **unveraendert** gruen bleiben, sonst ist es kein
   Durchsatzfix, sondern ein Verhaltensfehler.

## Abnahme

`ns/px` der Ball-Demo vorher/nachher auf derselben Maschine; die 59 Faelle der
Referenzsuite unveraendert; beide Boards bildpunktgleich.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
