# PORT07 — Quittung

**Status: UMGESETZT · in QEMU `raspi4b` gefahren und gemessen · Abnahme auf
echter Pi offen** · 2026-09-28 · `plat-newtos.c` geht jetzt denselben Weg wie
`plat-tt.c` und `plat-pistorm.c`: ein Speicher-MFDB und **ein `vro_cpyfm` je
Rechteck**. Kein XBIOS 2/3/5 mehr, kein Framebuffer-Zeiger. **Am NewTOS-Baum
ist keine einzige Quelltextzeile geändert worden.**

Messbericht: `measurements/measurement-2026-09-28-port07/` (RUNINFO.TXT, NUMBERS.TXT, fünf
Bildabzüge, fünf Demo-Protokolle, vier serielle Protokolle, kein `*.img`).

## Die Abnahmepunkte des Tickets

| Punkt | Ergebnis |
|---|---|
| Demo läuft sichtbar, ohne XBIOS 2/3/5 | ✅ QEMU `raspi4b`, 60,00 fps über 275 s, 1 verworfenes Bild von 16 500 (`measurement-.../raw/after-long-BALL.LOG`) |
| `/usr/bin/grep` findet keines der drei | ✅ `/usr/bin/grep -c -i -E "setscreen\|physbase\|logbase" plat-newtos.c` → **0** |
| Bildrate vorher und nachher, dieselbe Maschine | ✅ freilaufend **0,242 → 1,303 ms/Bild** (Faktor 5,39); Blit **4,38 → 80,49 ns/px** (Faktor 18,4) |
| `ball.c`/`ball_neu.c`/`ball-tex.c` unangetastet | ✅ nur `plat-newtos.c` |
| Kein Kerneleingriff | ✅ `git diff --stat` leer; Grundwerte alle unverändert |

## Was unterwegs herauskam, und den Vorher-Arm erst ermöglichte

**Die vorgefundene Plattformdatei startet auf HEAD `b9a2444` überhaupt
nicht.** Sie bricht mit `BALL: could not measure the screen pitch` ab — frisch
übersetzt ebenso wie das ausgelieferte Binärbild vom 2026-09-09
(`raw/before-unrepaired-serial.log:58`, `raw/shipped-2026-09-09-serial.log:58`).

Der Grund ist genau das, wogegen PORT06 argumentiert. Die Zeilenbreite wurde
gemessen, indem der Bildschirmanfang genullt, Zeile 1 per `vr_recfl` mit Stift
1 gefüllt und das erste geänderte Wort gesucht wurde — begründet mit *„every
VDI pen is opaque (alpha 0xFF), so any pixel it draws is non-zero"*. Am
2026-09-21 hat `2d6b3cc` („fix(vdi): the device pixel carries the Falcon
word's overlay bit") dem Bildpunkt sein unbedingtes `0xFF` im X-Byte genommen:

```
kernel/src/hal/framebuffer.rs, vor 2d6b3cc
    pub const fn rgb(r,g,b) -> u32 { 0xFF00_0000 | ((r)<<16)|((g)<<8)|(b) }
danach
    pub const fn rgb(r,g,b) -> u32 {              ((r)<<16)|((g)<<8)|(b) }
```

Stift 1 ist Schwarz, Schwarz ist seither das Nullwort, die Suche kann nicht
mehr anschlagen. **An der zugesagten Schnittstelle hat sich nichts bewegt** —
nur an einem Interna, von dem die Demo nie hätte abhängen dürfen. Sieben Tage
lang war die Demo tot, ohne dass es auffiel.

Für den Vorher-Arm war deshalb eine einzeilige Reparatur nötig (Stift 2 statt
Stift 1, `method/before-arm-repair.diff`). Sie steckt **nur** in einer
Scratch-Kopie; ausgeliefert ist die neue, konforme Datei.

## Was `plat-newtos.c` jetzt tut

* Der Rahmen geht in einen Speicher-MFDB in der Geräteform der NewTOS-VDI
  (`fd_stand = 0`, `fd_nplanes = 16`, ein Gerätewort je Pixel,
  `fd_wdwidth = (fd_w + 15) / 16` — `docs/api/vdi/raster/_raster.md`,
  `tos-api/src/vdi/raster.rs`) und von dort mit einem `vro_cpyfm` auf den
  Schirm.
* **Die Farbtabelle wird gemessen, nicht gerechnet.** `vs_color` setzt die 256
  Farben der Demo als VDI-Stifte, je Stift wird ein Pixel gemalt, die Zeile
  per `vro_cpyfm` Bildschirm→Speicher zurückgelesen. In der Datei steht keine
  RGB-nach-Wort-Formel. Das ist die Lesart „`vs_color` wird zur Umsetztabelle"
  aus dem Ticket.
* Die Form wird einmalig verifiziert (`probe_format`): bekanntes Muster
  stagen, mit demselben Aufruf blitten, den `plat_blit` benutzt, vollständig
  zurücklesen. Stimmt es nicht, weigert sich die Demo, statt Unsinn zu malen.
* Anders als die zwei Vorlagen sucht diese Datei **nicht** zwei
  `fd_wdwidth`-Konventionen ab. Sie nennt die, die NewTOS im eigenen Baum
  festlegt, und prüft sie nach. Zweiter Grund neben der Spezifikation: eine
  Form unter der anderen Konvention beschreibt einen sechzehnmal so großen
  Puffer — auf dem Atari harmlos, hier ein Lesezugriff über die Abbildung
  hinaus.
* Weg sind die Zeilenbreitenmessung, die Seitenumschaltung und jeder
  Framebuffer-Zeiger.

## Die Zahlen

QEMU `raspi4b`, 640×480, `hud 1`, drei Vorher- und zwei Nachher-Läufe:

```
freilaufendes Bild (ball.c bench(), 0,2 s ohne Vsync)
    vorher   813 / 819 / 848 Bilder   = 4065 / 4095 / 4240 fps   ~0,242 ms
    nachher  154 / 153 Bilder         =  770 /  765 fps          ~1,303 ms
                                                        Faktor   5,39

ein Pixel im Blit (ball.c blit_cost())
    vorher    4,42 / 4,42 / 4,30 ns/px  + 0,00 us/Aufruf
    nachher  82,41 / 78,57      ns/px  + 2,01 / 2,88 us/Aufruf
                                                        Faktor  18,4

dargeboten (Vsync-gebunden)
    vorher   75 Intervalle / 375 s, 60,00 fps,  4 verworfen von 22 500
    nachher  55 Intervalle / 275 s, 60,00 fps,  1 verworfen von 16 500
```

Der Blit ist damit von 28 % auf 86 % des freilaufenden Bildes gewachsen — er
**ist** jetzt das Bild. Auf dem Schirm sieht man davon nichts: der Vsync
deckelt beides bei 60 fps, und die Bildabzüge nach 430 s (254 Abpraller)
zeigen dieselbe Szene ohne Reste und ohne Nachziehen.

**Die Verschlechterung ist der Befund, nicht der Grund zurückzurudern** — das
Ticket sagt es so, und die Zahl ist genau die, die PORT06 gebraucht hat.

## Das Urteil für PORT06

**Dooms Bildweg ist belegt, nicht mehr angenommen.** Ein
Truecolor-Speicher-MFDB geht per `vro_cpyfm` auf den Schirm, Bild für Bild,
über Stunden, ohne `Physbase` und ohne `Setscreen`, und das Ergebnis ist von
der direkten Fassung mit bloßem Auge nicht zu unterscheiden.

Zwei Präzisierungen, die PORT06 §3a aufnehmen sollte:

1. **„Truecolor" heißt hier 15 Bit (5-5-5), nicht 24.** Das Gerätewort der VDI
   ist der Falcon-Pixel mit fünf Bit je Kanal, während der Framebuffer darunter
   acht hält. Belegt: von 159 Farben im Vorher-Bild sind 158 nicht in 5 Bit
   darstellbar, von 116 Farben im Nachher-Bild **keine einzige**
   (`raw/colour-depth.txt`). Dooms Palette wird also quantisiert — kein
   Hindernis, aber es gehört gewusst.
2. **Der Preis je Bild, auf dieser Maschine gemessen:** Dooms 320×200 =
   64 000 Pixel kosten **5,15 ms** allein im `vro_cpyfm` (18 % eines
   35-Hz-Taktes, 31 % eines 60-Hz-Bildes). Der Umrechnungsdurchlauf, den §3a
   auf „unter einer Millisekunde" veranschlagt hat, ist dabei der billige
   Teil — er verschwindet im Staging. Teuer ist das `vro_cpyfm` selbst.

## Befunde an der VDI — berichtet, nicht behoben

Auftragsgemäß wurde am Kern nichts angefasst. Zwei Stellen erklären den
Faktor 18 je Pixel und gehören in ein eigenes Ticket **nach** dem Termin:

* **V-a** `mem_to_screen` (`tos-api/src/vdi/raster.rs:494-533`) liest für jedes
  Pixel den lebenden Bildschirmpunkt und rechnet ihn nach RGB565 — auch bei
  `S_ONLY` (Op 3), wo `raster_op` das Ergebnis wegwirft. Beim häufigsten
  Kopiermodus ein Framebuffer-Lesezugriff plus eine Umrechnung je Pixel
  umsonst.
* **V-b** Die Schleife läuft über `&dyn RasterBackend`: zwei virtuelle Aufrufe
  (`get_device_pixel`, `put_device_pixel`) und zwei Umrechnungen je Pixel,
  Basis und Zeilenabstand je Pixel neu aus `active_fb()`.

Beides ist Durchsatz, nicht Verhalten — kein Bild wird dadurch falsch. Die
Ball-Demo ist der Messstand, der eine Behebung bewerten kann.

## Was nur die echte Pi zeigen kann

* **1920×1080.** QEMU meldet 640×480. Auf der Pi zeichnet die Demo ~134 000
  statt ~14 300 Pixel je Bild. Ob das im 16,7-ms-Budget bleibt, sagt nur die
  Pi. Die Hochrechnung (≈ 4,1 ms/Bild) steht in `NUMBERS.TXT` §5.2 **und ist
  ausdrücklich keine Messung**.
* **Der saubere Ausstieg.** Die Demo endet auf einen Tastendruck; `raspi4b`
  hat keine Tastatur und verwirft serielle Zeichen. Die Läufe hier wurden
  abgeschossen; Abschlusszusammenfassung, Palettenwiederherstellung und die
  neue `BALL: NewTOS screen …`-Zeile aus `plat_close()` sind damit **nicht**
  belegt.
* **Der Cache.** QEMU hat kein Cachemodell und macht aus dem nicht-cachenden
  Framebuffer gewöhnlichen Hostspeicher — genau die Kosten, die auf der Pi
  zählen, fehlen hier.

## Grundwerte, nachgeprüft (unverändert, weil kein Kernel-Code angefasst)

```
cargo run -p xtask -- test        961 passed / 0 failed / 1 ignored
cargo xtask build --board rpi4     tos-api 64 warnings, kernel 94, rc=0
cargo xtask build --board qemu-virt tos-api 64 warnings, kernel 94, rc=0
cargo clippy -p tos-api            80 warnings
cargo clippy -p kernel (rpi4)     125 warnings
cargo clippy -p kernel (qemu-virt) 125 warnings
sh tools/desk-drift-probe.sh       DRIFT-PROBE TOTAL: 32
git status --short                 nur docs/analysis/emutos-switches.txt, measurements/hw-round-2026-09-24/
                                   und dieses Messverzeichnis
```
