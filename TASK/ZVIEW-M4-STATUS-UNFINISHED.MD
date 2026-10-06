# ZVIEW-M4 — Quittung (UNFERTIG): zView als ZVIEW.NAP + zehn Codecs `.NLD`, ohne PDF — blockiert durch Kernel-Befund K1

Datum 2026-10-07 · Basis `desk-rebuild` `353777d` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-06-zview-m4](../measurements/measurement-2026-10-06-zview-m4/RUNINFO.TXT)
· Doku: [docs/sdk/13-zview.md](../docs/sdk/13-zview.md)

## Was steht

* **Bau** `sdk/zview/build-zview.sh`: zView (LDG-Fassung, z-tools `04805b7`,
  gepinnt als `sdk/zview/z-tools-04805b7.tar.xz`) → `ZVIEW.NAP`; Codecs bmp,
  degas, eureka, gif, godpaint, img, jpg, neochrome, png, tga → `CODECS\*.NLD`.
  0 undefinierte Symbole, jede Codec-Link-Map nur mit libshare-Allokator,
  zwei Bauläufe bytegleich. tiff nicht gebaut (libtiff + liblzma fehlen), PDF
  weggelassen (zwei Aufrufstellen per Overlay entfernt, keine Attrappen).
* **Quellen unverändert**, 12 Overlays mit Kopfkommentar (Datei:Zeile, Grund):
  `.NLD`-Suche, PDF-Auslassung (2), VA_START hi/lo, Big-Endian-Header/Paletten/
  Pixel (degas, neochrome, godpaint, eureka, img), zwei m68k-Asm-Retargets
  (`img/xp32.S` → `xp32.c`, `jpg/jpgdsp.c`-Makros), LP64 in `jpgldg.c`.
  Prelude `zview-ints.h` über `types2b.h`s eigenen `HAVE_INTS_DEFINED`-Haken.
* **Karte** (nur QEMU): `tools/make-sd-image.sh` installiert `C:\ZVIEW\` mit
  `CODECS\`, `ICONS\`, `SAMPLES\` (15 synthetische Muster,
  `sdk/zview/samples/gen-samples.py`).
* **Lauf**: zView startet über `shel_write` (Startverzeichnis wie EmuTOS),
  lädt alle zehn Codecs, dekodiert die Datei der Kommandozeile, öffnet das
  Fenster mit richtiger Infozeile.
* **Pixelprüfung** mit Diagnose-Bau (nur `app.nplanes`/`app.color` nach K1
  ergänzt): 15/15 Muster pixelgenau (memprot on), 4/4 unter strict; reales Foto
  (privat, nur Scratch) 9/10 pixelgenau, das RLE-TGA scheitert am
  Upstream-Fehler U1.
* **Regression**: APITEST 106/0/5, QED und TeraDesk starten, Bilder
  pixelgleich zu M3. SDK-Host-Tests 16/0/0.

## Blocker K1 (Kernel, nicht angefasst)

`graf_handle` liefert Handle 1 (EmuTOS `gl_handle`), NewTOS öffnet VDI-Handle 1
nie (`kernel/src/desk/vdibind.rs:16-26`), `vq_extnd(1, …)` schreibt nichts
(`VQPROBE`). WinDom-`ApplInit` (`appl_init.c:42-46`) setzt damit
`app.nplanes = app.color = 0` in jedem WinDom-Programm. Folge in zView:
Bild-MFDBs mit 0 Planes → graues Fenster; Bilder breiter als 128 px
überschreiben den Heap → Bus Error (gemessen, 320×200 DEGAS, WinDom
`DataSearch`). EmuTOS öffnet die physikalische Workstation beim AES-Start
(`aes/gemgsxif.c:169-185`). Die Korrektur gehört in den Kernel — Entscheidung
und Umsetzung beim Lead.

## Offen

* K1 beheben, dann `bin/ZVIEW/ZVIEW.NAP` (Produkt) mit
  `method/zv-runs.sh on` / `strict` + `check-all.sh` erneut prüfen.
* U1 (Upstream, `tgaldg.c fill_img_buf`): dokumentiert, nicht geändert.
* Nicht gefahren: Browser, Dateiauswahl, Speichern/Encoder, Animation,
  Diashow, AV, memprot=off, echte Pi.
* `docs/STATUS.md`-Eintrag (Lead).
