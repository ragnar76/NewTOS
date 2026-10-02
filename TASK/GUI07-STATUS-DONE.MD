# GUI07 — Status: **DONE (QEMU)**, Pi-Abnahme offen

Basis: `desk-rebuild` @ `234564f`, nicht committet. Belege:
[measurements/measurement-2026-10-02-gui07/](../measurements/measurement-2026-10-02-gui07/RUNINFO.TXT)
(Diff: `method/gui07-change.diff`).

**Offen, vorweg:** Alle Zahlen sind QEMU-TCG-Zahlen und nur relativ. Die
Abnahme „`ns/px` vorher/nachher auf derselben Maschine“ ist in QEMU erfüllt;
auf der echten Pi 4 steht sie aus (siehe §6).

## 1. Wohin die Zeit ging

Gemessen mit drei Kernen, sonst gleichen Binärdateien: *before*, *s1*
(Scratch, nur Stufe 1: S_ONLY liest den Schirm nicht mehr, schreibt aber
weiter je Pixel über zwei virtuelle Aufrufe) und *after*.

| | before | s1 | after |
|---|---|---|---|
| Ball-Demo 640x480, ns/px | 82,37 | 19,66 | **12,68** |
| Ball-Demo 1920x1080, ns/px | 78,40 | 19,15 | **11,64** |
| Doom x4 (1280x960), `vro_cpyfm` je Bild | 90 909 µs (74,0 ns/px) | 16 666 µs | **7 730 µs (6,3 ns/px)** |
| Doom x2, `vro_cpyfm` je Bild | 23 928 µs | 4 115 µs | **1 879 µs** |
| Doom x1, `vro_cpyfm` je Bild | 4 830 µs | 848 µs | **379 µs** |
| timedemo demo1 x1 / x2 / x4 (1920x1080) | 141,8 / 38,1 / 10,0 fps | – | **411,0 / 206,7 / 90,6 fps** |

* **~82 % der Pixelkosten** lagen im Lesen des Schirmpunkts, dessen Ergebnis
  `S_ONLY` wegwirft (V-a): `get_device_pixel` + `device_to_rgb565`, je ein
  virtueller Aufruf, jeder davon löst `active_fb()` auf (`&dyn Framebuffer`,
  vier Acquire-Lasten für Breite/Höhe/Zeilenabstand/Basis) und prüft Grenzen.
* Der Rest halbiert sich durch Stufe 2 (V-b): Basis, Zeilenabstand und
  Grenzen einmal je Lauf, ein virtueller Aufruf je 256 Pixel statt zwei je
  Pixel.
* Der feste Anteil je Aufruf stieg von 2,4–3,0 auf 4,0–4,1 µs (Stapelpuffer,
  Laufaufteilung). Bei 6 ns/px sind das rund 330 Pixel – ein Befund, kein
  Fehler.

## 2. Was sich geändert hat

* `tos-api/src/vdi/backend.rs`: `RasterBackend::read_row_rgb565` /
  `write_row_rgb565`. Die Default-Implementierung ist **genau** die
  Pro-Pixel-Definition (`device_to_rgb565(get_device_pixel(..))`,
  `put_device_pixel(.., rgb565_to_device(..))`, auch die Regel für Pixel
  außerhalb des Schirms). Testdoppel brauchen also keine Änderung.
* `kernel/src/vdi_backend.rs`: `VdiScreen` überschreibt beide Methoden. Der
  Framebuffer wird einmal je Lauf aufgelöst, die Grenzen einmal je Lauf
  geprüft (`row_span`), dann eine enge Umrechnungsschleife.
  Normale (nicht-`volatile`) Lade- und Speicherzugriffe: der Speicher ist
  Normal Memory (WB seit GUI06, RAM hinter ramfb), und die Cachepflege in
  `flush_rect` ist ein `asm!` ohne `nomem`, also eine Compiler-Barriere.
  Die Draw-Logik bleibt hinter dem Trait. Ein späterer NEON/asm-Tausch
  betrifft nur das Backend.
* `tos-api/src/vdi/raster.rs`:
  * `op_ignores_dst` = {0, 3, 12, 15}, `op_ignores_src` = {0, 5, 10, 15}.
    Die Mengen sind **aus der Wahrheitstabelle von `raster_op` abgeleitet**,
    nicht gewählt: Es sind genau die Arme, in denen `d` bzw. `s` nicht
    vorkommt. Ein Hosttest prüft das für alle 32 Op-Werte. Sie decken sich
    mit EmuTOS `vro_cpyfm16` (`vdi_raster.c:1009-1019, 1048-1068, 1167-1180,
    1209-1219`, kein `*q` gelesen).
  * `mem_to_screen`: je Zeile und 256-Pixel-Lauf: Quelle per
    `Mfdb::read_row` holen. Den Schirm **nur** lesen, wenn die Op ihn
    braucht. `apply_row`, das ist `raster_op` mit Op als Konstante, eine
    Schleife je Op. Danach ein `write_row_rgb565`. `BM_D_ONLY` behält sein
    bisheriges Lesen und Zurückschreiben (Kanonisierung bleibt).
  * `screen_to_mem`: je Zielzeile nur die Spalten, die der Puffer annimmt
    (`row_window`). Schirmlauf per `read_row_rgb565`, dann
    `Mfdb::combine_row`. Liegt eine Koordinate über `i16::MAX`, wird
    `device_to_rgb565(0)` geliefert, wie früher über den gekappten Cast.
  * `mem_to_mem`: dieselbe Zeilenreihenfolge wie bisher. Innerhalb einer
    Zeile gilt: Sind die gelesenen Quellbytes und die geschriebenen Zielbytes
    disjunkt, wird in Läufen kopiert. Sonst läuft die Zeile pro Pixel in der
    alten Spaltenrichtung. Dadurch bleiben auch „verschmierende“
    Überlappungen (versetzte Puffer, gemeinsamer Puffer seitwärts)
    wortgleich.
  * `BM_D_ONLY` mit Speicherziel kehrt sofort zurück. Das ist wortgleich,
    weil jedes Schreiben das eben gelesene Wort zurückschrieb, und es
    entspricht EmuTOS „nothing to do“ (`vdi_raster.c:1083-1085`).
  * `put_mem`/`get_mem` bleiben für `mono_to_mem` und den Pro-Pixel-Zweig.

## 3. fVDI als Gegenlesung (`drivers/16_bit/16b_blit.c` @ 5a1ce94)

* **Gefolgt:** die Spezialisierung je Op. fVDI schreibt eigene Schleifen für
  `S_ONLY` (`blit_copy`) und `S_OR_D` (`blit_or`), dazu eine generische mit
  `DO_OP` (`blit_16b`, :795). Hier erzeugt `apply_row_op::<OP>` für jede der
  16 Ops eine eigene Schleife. Bei fVDI liest `blit_copy` das Ziel ebenso
  wenig wie unser `S_ONLY`.
* **Bewusst anders:** Die Richtungsregel. fVDI kopiert von unten nach oben,
  wenn `src_y < dst_y` (:1150-1155), und rückwärts nur innerhalb derselben
  Zeile (`pan_backwards_*`, :1173). EmuTOS entscheidet über `src < dst`
  (`vdi_raster.c:995-1002`). NewTOS behält seine bestehende Regel (gleicher
  Puffer: abwärts bei `dy > s.y`, rückwärts bei `dx > s.x`). Der Auftrag
  ist reiner Durchsatz, eine Regeländerung wäre eine Verhaltensänderung.
* **Nicht übernommen:** fVDIs Schattenpuffer (`s_blit_*`). Er umgeht
  langsames Lesen des Schirms über den Bus. Bei uns ist der Framebuffer seit
  GUI06 WB-gecacht, und das Lesen entfällt für die D-unabhängigen Ops ohnehin.
* `blit_16b` liest das Ziel auch für 0/12/15. Wir lesen es dort nicht.

## 4. Verhalten unverändert – Belege

* DUMPRAST im echten Kern (62 Fälle R02–R05): Falcon-TC gegen before 0/62,
  gegen after 0/62. `RASTER.BIN` before/after **byte-gleich**.
* Doom in allen Läufen aller Arme: EdDI 256/256 exakt, „VERIFIED“ für jedes
  Ausgabepixel bei x1/x2/x4.
* Screendumps: Desktop 640/1080 identisch, ebenso das Doom-Titelbild, x1
  und der Desktop nach Doom. Die zeitgetriebenen Doom-/Ball-Bilder
  unterscheiden sich erwartungsgemäß: Der schnellere Kern zeigt zur selben
  Wanduhrsekunde ein späteres Bild.
* Host: `cargo test -p tos-api -p tos-types -p xtask --no-fail-fast` ergibt
  **980/0/1** (975 + 5 neue). `vdi_raster_reference` 59/59 unverändert grün.
  Die neuen `gui07_tests` vergleichen alle 32 Op-Werte gegen die alten
  Pro-Pixel-Funktionen, die als Orakel wörtlich im Testmodul stehen. Abgedeckt
  sind: unbeschnitten, beschnitten, ganz weggeschnitten, Schirmränder,
  Breiten 1/3/17/255/256/257/513/700/701, Quelle bzw. Ziel teilweise
  außerhalb, `fd_wdwidth` zu klein oder 0, ungerade Pufferadressen,
  Koordinaten über `i16::MAX` sowie alle Überlappungsarten. Der Schirm ist
  mit beliebigen, auch nicht-kanonischen 32-Bit-Worten gefüllt. Jeder Test
  läuft gegen die Trait-Defaults und gegen eine Override-Variante in
  Kernel-Form. Drei absichtliche Mutationen wurden alle gefangen.

## 5. Grundwerte

rustc Release beide Boards 64/94 → 64/94 · clippy 80/125 → 80/125 (keine
Meldung in den drei Dateien) · `desk-drift-probe` 31 → 31.

## 6. Was auf der Pi zu messen ist

Kern frisch mit `cargo xtask build --board rpi4 --release`. Dann mit
derselben Ball-Demo (`hud 1`) und demselben Doom-Bau wie im Pi-Lauf vom
2026-10-02:

1. `C:\BALL.LOG`, Zeile `BALL: blit … ns/px`. Vorher wurden auf der Pi rund
   190 ns/px gemessen.
2. Doom ohne Parameter (x4) und mit `-scale 3`: die Zeile `DOOM: per frame
   …`. Sie liefert `vro_cpyfm` in µs, ns/px = µs·1000/Pixelzahl. Dazu die
   5-s-fps-Zeile. Vorher 4,2 fps (x4) und 7,6 fps (x3).
3. Sichtprüfung: Desktop, Menü auf/zu (`bb_save`/`bb_restore`),
   Farbsymbole (`gr_colourblit`, Op 1). Alles muss aussehen wie vorher.

Eine Vorhersage für die Pi gebe ich bewusst nicht. TCG bildet weder die
Caches noch die Vektorisierung der neuen Schleifen ab.

## 7. Offene Fragen / Grenzen

* **Ein MFDB, dessen `fd_addr` in den Bildschirmspeicher selbst zeigt.**
  Dann überlappt ein 16-Bit-Speicherformular den 32-Bit-Framebuffer. Für
  `mem_to_screen`/`screen_to_mem` liest der neue Pfad einen Lauf von bis zu
  256 Pixeln, bevor er ihn schreibt. Die alte Schleife wechselte je Pixel
  ab. Nur in diesem Fall könnte ein Ergebnis abweichen. Erreichbar wäre er
  nur über eine EL0-Alias-Adresse von `Logbase`, also eine andere VA als die
  des Kerns, und physikalisch sinnlos ist er obendrein (Formate verschieden).
  Behandelt habe ich ihn nicht, ich lege ihn hier offen. `mem_to_mem` ist
  davon nicht betroffen: Dort ist jede Überlappung exakt.
* Der feste Aufrufanteil (+1–1,5 µs in QEMU) ließe sich senken, etwa durch
  kleinere oder nicht genullte Puffer. Das gehört nicht zu diesem Ticket.
* Für die nächsten Stufen des Tickets (NEON-Intrinsics, asm) ist der Ort
  jetzt eng umrissen: die zwei Kernel-Schleifen in `read_row_rgb565` /
  `write_row_rgb565` und `apply_row_op`.
