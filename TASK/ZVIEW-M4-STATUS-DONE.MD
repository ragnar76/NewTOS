# ZVIEW-M4 — Quittung (FERTIG): zView als ZVIEW.NAP + zehn Codecs `.NLD`, ohne PDF

Datum 2026-10-07 · Basis `desk-rebuild` `81d0949` + K1 (nicht committet)
· Vorgeschichte: [ZVIEW-M4-STATUS-UNFINISHED.MD](ZVIEW-M4-STATUS-UNFINISHED.MD)
  (Bau, Overlays, Karte, Diagnose; bleibt als Verlauf stehen)
· Blocker K1: [ZVIEW-M4-K1-STATUS-DONE.MD](ZVIEW-M4-K1-STATUS-DONE.MD)
· Beleg Abnahme: [measurement-2026-10-07-k1](../measurements/measurement-2026-10-07-k1/RUNINFO.TXT)
· Beleg Bau: [measurement-2026-10-06-zview-m4](../measurements/measurement-2026-10-06-zview-m4/RUNINFO.TXT)
· Doku: [docs/sdk/13-zview.md](../docs/sdk/13-zview.md)

## Abnahme mit dem Produkt-Bau

Geprüft wurde `ZVIEW.NAP` aus `sdk/zview/build-zview.sh`, unverändert
(sha256 `9c6497ad…`, bytegleich zu `measurement-2026-10-06-zview-m4/bin/ZVIEW`).
Kernel mit K1, QEMU raspi4b, `serial=on`.

* **memprot on: 15/15 pixelgenau.** T24.BMP, T8.BMP, T24.TGA, TRLE.TGA,
  TRGB.PNG, TPAL.PNG, T.GIF, T.JPG, T.PI1, T.PC1, T.NEO, T.GOD, T.RAW,
  T4.IMG und T1.IMG, jeweils als ganzes Bild gegen die Referenz verglichen
  (`checks/on-shotcheck.txt`).
* **memprot strict: 3/3 pixelgenau** für T.JPG, T.PI1 und T4.IMG
  (`checks/strict-shotcheck.txt`).
* Jeder Lauf endet mit rc 0, kein Absturz im seriellen Log. T.PI1
  (320×200 DEGAS) zeigt das Bild. Im M4-Produktlauf brach es mit Bus Error ab.

Die Diagnose-Fassung `ZVIEW-K1DIAG.NAP` wird nicht mehr gebraucht. Das
Produkt liest `app.nplanes`/`app.color` jetzt selbst über WinDoms
`ApplInit` vom `graf_handle`-Handle.

## Offen (unverändert aus M4)

* U1 (Upstream, `tgaldg.c fill_img_buf`): dokumentiert, nicht geändert.
* Nicht gefahren: Browser, Dateiauswahl, Speichern/Encoder, Animation,
  Diashow, AV, memprot=off, echte Pi.
* tiff (libtiff + liblzma fehlen) und PDF: nicht gebaut.
* `docs/STATUS.md`-Eintrag: Lead.
