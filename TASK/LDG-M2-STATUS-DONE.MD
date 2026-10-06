# LDG-M2 — Quittung: Fremdbibliotheken der zView-Codecs als statische NewTOS-Bibliotheken

Datum 2026-10-06 · Basis `desk-rebuild`, HEAD `6a925cc`, Baum vorher sauber
· nicht committet · Belege [measurements/measurement-2026-10-06-ldg-m2/](../measurements/measurement-2026-10-06-ldg-m2/RUNINFO.TXT)
· Grundlage [docs/analysis/zview-ldg-2026-10-04.md](../docs/analysis/zview-ldg-2026-10-04.md) §1–2,
[docs/analysis/ldg-after-arch03-2026-10-06.md](../docs/analysis/ldg-after-arch03-2026-10-06.md)

Umfang: was die fünf Demo-Codecs brauchen. BMP/TGA keine Fremdbibliothek;
GIF → giflib; PNG → libpng + zlib (+ libm, vorhanden); JPEG → libjpeg +
**libexif** (`plugins/jpg/Makefile: -ljpeg -lexif -lm`, `jpgldg.c:374-458`).
Daher fünf Bibliotheken, nicht vier.

## Umsetzung

| Bibliothek | Version | Tarball (Herkunft/Prüfung in `<lib>-sources.sha256`) | Archiv | Konfiguration |
|---|---|---|---|---|
| zlib | 1.3.2 | `sdk/zlib/zlib-1.3.2.tar.xz` 1,32 MB + `.asc` (Signatur gut) | `libz.a` 119 582 B | `zconf.h` aus `zconf.h.in` mit den zwei sed-Zeilen von `configure` |
| libpng | 1.6.59 | `sdk/libpng/libpng-1.6.59.tar.xz` 1,07 MB (SHA-1 = SourceForge) | `libpng.a` 299 780 B | `scripts/pnglibconf.h.prebuilt` (INSTALL); NEON-Filter an (Upstream-Vorgabe) |
| libjpeg (IJG) | 9f | `sdk/libjpeg/jpegsrc.v9f.tar.gz` 1,08 MB (IJG veröffentlicht keine Prüfsumme) | `libjpeg.a` 310 244 B | `overlay/jconfig.h`, `jmemnobs` |
| giflib | 6.1.3 | `sdk/giflib/giflib-6.1.3.tar.gz` 0,47 MB (SHA-1 = SourceForge) | `libgif.a` 46 778 B | keine |
| libexif | 0.6.26 | `sdk/libexif/libexif-0.6.26.tar.xz` 1,34 MB + `.asc` (Signatur gut) | `libexif.a` 389 618 B | `overlay/config.h` (kein NLS, `localtime_r`) |

* **Quellen unverändert.** Was sonst `configure` schreibt, liegt als
  kommentierter Konfigurationskopf im `overlay/` (libjpeg, libexif) bzw. wird
  mit Upstreams eigenem Schritt erzeugt (zlib, libpng). Quellenlisten werden
  aus den eigenen Makefiles der Bibliotheken gelesen. Jede nicht übersetzbare
  Datei stoppt den Bau.
* **Bau:** `sdk/<lib>/build-<lib>.sh`, gemeinsame Schritte in
  `sdk/vendor-lib.sh`; Flags = SDK-Freestanding-Satz mit `-fPIE` + die
  Warnflags der jeweiligen Bibliothek. In `sdk/lib-stale.sh` als `zlib`,
  `libpng` (Kette libc → zlib → libm → libpng), `libjpeg`, `giflib`,
  `libexif` (libc → libm → libexif); Tarball, Pin-Datei, Overlay, Skript und
  `vendor-lib.sh` beobachtet. `sdk/.gitignore` erweitert.
* **Test** `sdk/examples/libtest/`: `libtest.c` (38 Prüfungen, Host und
  Ziel aus einer Quelle), `gen-data.py` (14 Testdateien + Python-Referenzen
  → `libtest-data.h`), `make-ref.sh` (dieselben Quellen für den Host gebaut
  → `libtest-ref.h`; `--check` = SDK-Host-Test), `build-libtest.sh`
  (`LIBTEST.NCP`).
* **SDK-Host-Tests:** `sdk/run-host-tests.sh` um `libtest-host` ergänzt.
* **Doku:** `docs/sdk/11-codec-libraries.md` (Versionen, Pins, Linkzeile,
  Bedarf je Codec, Test), `docs/sdk/README.md`.
* **Kernel, tos-api, libc: nicht berührt.**

## Abnahme

* Bau: 15/18/46/8/25 Dateien, 0 Fehler, warnfrei bis auf eine
  Upstream-Warnung in libexif (`-Wswitch`, dokumentiert). Zwei frische Bauten
  bytegleich; alle externen Referenzen lösen in libc/libm/libz auf;
  `lib_ensure` konvergiert.
* QEMU raspi4b, `\AUTO\LIBTEST.NCP`: **`libtest: ALL PASS (38 checks)`** unter
  memprot **on** und **strict**. Alle 16 Python- und 24 Host-Referenzen
  bitgleich getroffen.
* SDK-Host-Tests **15/0/0** (vorher 14/0/0; neu `libtest-host`).
  Gegenprobe: manipulierte Referenz → `--check` schlägt fehl.
* Rust-Prüfungen nicht nötig (kein Rust berührt).

## Offen / für M4

* `libshare` (alle Codecs), Codec-Overlays. Syntaxprobe der unveränderten
  `pngldg.c`/`gifldg.c`/`jpgldg.c` gegen die neuen Köpfe: keine
  Bibliotheks-API-Abweichung; `jpgldg.c` speichert `FILE *` ohne Cast in
  `int32 _priv_var` (`:469,592,702,750`) — LP64-Fall fürs Codec-Overlay.
* Weitere Bibliotheken, falls mehr als die Demo-Codecs: **tiff** → libtiff +
  liblzma (+ libjpeg, zlib vorhanden); **zView-PDF** → freetype + xpdf (C++).
  Die übrigen Codecs (degas, neochrome, eureka, godpaint, img) brauchen
  keine; img hat m68k-Assembler (`xp32.S`).
* `LIBTEST.NCP` ist nicht in `tools/make-sd-image.sh` eingetragen (nicht
  verlangt; die Läufe nutzen `method/seed-card-libtest.sh`).
* `docs/STATUS.md`-Eintrag steht aus (Lead, wie bei M1).
