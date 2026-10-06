# LDG-M3 — Quittung (FERTIG): WinDom 1.21.3 als SDK-Bibliothek, LDG-Demo-Client mit `EX_LIB.NLD` und `POPUP.NLD`

Datum 2026-10-06 · Basis `desk-rebuild` · Abschluss nach LDG-M3-B1 und LDG-M3-B2
· Vorgeschichte (bleibt): [TASK/LDG-M3-STATUS-UNFINISHED.MD](LDG-M3-STATUS-UNFINISHED.MD)
· Belege: [measurement-2026-10-06-ldg-m3](../measurements/measurement-2026-10-06-ldg-m3/RUNINFO.TXT) (WinDom, Client),
[measurement-2026-10-06-aes-evbutton](../measurements/measurement-2026-10-06-aes-evbutton/RUNINFO.TXT) (B1),
[measurement-2026-10-06-libshare](../measurements/measurement-2026-10-06-libshare/RUNINFO.TXT) (B2, Abnahme)

## Was M3 offen ließ und wie es geschlossen ist

Die UNFERTIG-Quittung hatte den Weg zu `sinus()`/`cosinus()` über die
Oberfläche durch zwei Befunde blockiert gemeldet:

* **B1 (Kernel-AES, `EV_MB` = lebender statt Ereignis-Tastenzustand):**
  behoben in [LDG-M3-B1](LDG-M3-B1-STATUS-DONE.MD) — ein normaler Klick auf
  ein WinDom-Formular wirkt.
* **B2 (Modul-Heap nach Modul-`exit`):** behoben in
  [LDG-M3-B2](LDG-M3-B2-STATUS-DONE.MD) — jedes `.NLD` linkt libshare 0.1.1
  (Lizenz unklar, User fragt beim Autor nach).

## Abnahme (QEMU raspi4b, unveränderter Demo-Client)

`#Z C:\LDGDEMO\CLIENT\LDGDEMO.NAP`, Eingaben per Scratch-IKBD-Injektor
(`measurements/measurement-2026-10-06-libshare`, Läufe `popup` und `accept2`):

* Alt+C → „Example of math library!!!“ (`EX_LIB.NLD` `information()`);
  kurzer Mausklick auf *Compute* öffnet dieselbe Meldung (B1).
* Alt+F → `POPUP.NLD` `__popup`: Pop-up „Informations / Sinus / Cosinus“
  erscheint, Wahl *Sinus*, Wert 0.5, Compute → **0.479426**; Wahl *Cosinus*
  → **0.877583** — `EX_LIB.NLD`s `sinus()`/`cosinus()` über die Oberfläche.
* Return → Close → `AP_TERM` → `ldg_close` ×2 → `exit(0)`, Desktop.
* `memprot=on`, `strict` und `off`: alle Bilder pixelgleich; kein Bus
  Error, kein Hänger.
* LDGTEST 69/69 (off/on/strict); Regression APITEST 106/0/5, QED, TeraDesk.

## Offen (nicht Teil von M3)

* Im zweiten Pop-up ist „Informations“ leer — `client.c`-`strcpy` über das
  Ende von „Info“ hinaus, gleich auf dem Original (Quittung B2).
* `RsrcXload` ungetestet; `appl_tplay`-Records unbeobachtbar (A45).
* Für M4 (zView): `plugins.c`-Overlay für `.NLD`; Codecs linken libshare
  wie POPUP; `LdgQuit`-LP64-Stelle ohne TSR unerreichbar.
* Echte Pi nicht gefahren; `docs/STATUS.md`-Eintrag (Lead).
