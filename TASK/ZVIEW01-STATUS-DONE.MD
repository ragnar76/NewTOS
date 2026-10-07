# ZVIEW01 — Quittung (FERTIG, QEMU; Pi-Nachweis offen): zView-Browser zeichnet wieder Datei- und Ordnernamen

Datum 2026-10-07 · Basis `desk-rebuild` `428ac4a` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-zview01](../measurements/measurement-2026-10-07-zview01/RUNINFO.TXT)
· Ticket: [ZVIEW01](ZVIEW01.md)

## Ursache (belegt)

Keine Abweichung des Kerns von EmuTOS. zView zeichnet jeden Namen im Browser
(Kacheln `catalog/catalog.c:857,889,920,993,1020`, Baum
`catalog/catalog_mini_entry.c:192,195`) mit seinem eigenen Zeichensatz:
`draw_text()` (`zview/custom_font.c:159-205`) legt pro Zeichen eine
1-Ebenen-MFDB (8 WORDs) an und kopiert die 8 Glyphenzeilen **byteweise** aus
der WORD-Tabelle `my_font_8[]` jeweils in das erste Byte jedes WORDs
(`:182-183`, `:188-195`). Auf dem 68k ist das das High-Byte = Pixel 0..7.
Auf AArch64 (little-endian) landet jede Zeile in Bit 7..0 (Pixel 8..15), die
Zeilenpaare sind zusätzlich vertauscht. `vrt_cpyfm` kopiert nur die Spalten
`0..ofwf[c]-1` (2..8 Pixel) — dort ist alles leer, also wird nichts gezeichnet.

Der VDI liest die Form wie EmuTOS als WORDs in Maschinenreihenfolge, Bit 15 =
linkes Pixel (`tos-api/src/vdi/backend.rs` `MonoSrc`; EmuTOS
`vdi/vdi_raster.c:1241-1243`, `vrt_cpyfm16`). Das ist der richtige Vertrag;
das Problem ist ein Endianness-Trick in der App (Klasse B11 in
`docs/sdk/05-abi-pitfalls.md`).

Belege: Host-Sonde um das **unveränderte** `draw_text()` („ZVIEW“: jedes
Zeichen „NO PIXELS“, Worte `0004 007c …`); Hosttest über alle 255 Zeichen
rot gegen das Original (1816 Abweichungen, 0 Zeilen mit Pixeln), grün mit
Overlay; QEMU-Nachstellung zeigt genau das Bild des Anwenderfotos.

## Fix

Toolchain-Overlay, zView bleibt unverändert:
`sdk/zview/overlay/zview/custom_font.c` — `draw_text()` setzt
`fx_mem[row] = Zeilenbyte << 8` aus High-/Low-Byte von `my_font_8[k + row/2]`,
also genau die WORDs, die der 68k-Code erzeugt. Dazu Hosttest
`sdk/zview/tests/custom-font-host.c` (+ `general.h`-Ersatz), eingehängt in
`sdk/run-host-tests.sh`; Doku `docs/sdk/13-zview.md` §3 (13 Overlays),
`docs/sdk/README.md`, Kommentar in `sdk/zview/build-zview.sh`.
Neues `ZVIEW.NAP`: sha256 `cebb3b1b…` (vorher `9c6497ad…`).

## Prüfungen

* QEMU raspi4b, memprot on: vorher keine Namen
  (`screendumps/before-Z05_TREE.png`), nachher Namen schwarz in Kacheln und
  Baum, Größen grau, Auswahl weiß auf Grau (`screendumps/after-Z05_TREE.png`,
  `after-Z03_LATER.png`).
* Regression memprot on (LDGDEMO auch strict): EmuDESK, TeraDesk, QED,
  LDGDEMO, APITEST pixelgleich zu measurement-2026-10-07-k1, einzige
  Abweichung die Dateizeiten in der EmuDESK-Textansicht; APITEST 106/0/5.
  Der Kern ist unverändert (Release-Abbild = HEAD).
* Hosttests 1201/0/1 (inkl. VDI-/AES-Referenzsuiten), Release-Builds beide
  Boards 64/94, Clippy 76/125, Drift 31 — alles = Basis. SDK-Hosttests 17/0/0
  (Basis 16 + `custom-font-host`).

## Offen

* **Nachweis auf der echten Pi 4** (neues `ZVIEW.NAP` auf die Karte, nur auf
  Ansage).
* Doppelklick auf eine Ordnerkachel öffnet den Ordner im QEMU-Injektorlauf
  nicht (zweiter Druck 3 bzw. 72 Ticks gehalten; `samples-*`,
  `samples2-after-*`); der Klick wählt nur aus. Nicht untersucht, eigener
  Punkt. zView prüft dafür `nb_click == 2 && evnt.mbut == 1`
  (`catalog/catalog_mouse.c:132`, `evnt.bclick = 258` in `main.c:143`).
* `docs/sdk/13-zview.md` beschreibt K1 noch als offenen Blocker (Kopfnotiz,
  §6), obwohl K1 behoben ist — nicht angefasst.
* `docs/STATUS.md`-Eintrag gehört zum Commit (Lead).
