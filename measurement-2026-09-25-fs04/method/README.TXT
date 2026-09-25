FS04 — method/: drei Fassungen der Sonde, alle vollstaendig abgelegt
=====================================================================

Ein `.diff` ist ein kompletter Patch und braucht das Erzeugerskript nicht.
Wo es eines gibt, steht es dabei.

FASSUNG 1 — nur seriell
-----------------------
  fs04-probe-640.diff     Treiberbetrieb, Faelle A-G, 640x480
  fs04-probe-1920.diff    dasselbe mit erzwungener 1920x1080-Geometrie
  fs04-gesture.diff       Gestenbetrieb + IKBD-Injektor

  Damit aufgenommen:
    ../raw/r03-driver-640x480.log
    ../raw/r04-driver-1920x1080.log
    ../raw/g05-gesture-640x480.log
    ../raw/p01-probe-clickpoints.log
  Die Zahlen in ../RUNINFO.TXT und ../NUMBERS.TXT stammen aus dieser Fassung.

FASSUNG 2 — Bericht zusaetzlich in eine Datei auf der Karte
-----------------------------------------------------------
  fs04_probe_patch.py     das Erzeugerskript (Ausgangsstand fuer Fassung 3)
  fs04-pi-handover.diff   Gestenbetrieb OHNE Injektor
  fs04-gesture-r2.diff    Gestenbetrieb MIT Injektor

  Damit aufgenommen:
    ../raw/pi03-boot-of-handover-image.log
    ../raw/g07-gesture-report-to-card.log
    ../raw/g07-FS04.TXT
    ../raw/pi04-FS04.TXT   (der Lauf auf der echten Pi)

  Neu gegenueber Fassung 1: Bericht nach C:\FS04.TXT (angehaengt), Startblock
  mit Bildschirmgeometrie, `dos_seek` in desk/dosbind.rs,
  `vdi_backend::fs04_fb_geometry()`.

FASSUNG 3 — umgestellt auf den neuen EMMC-Treiber (c6cf484)
------------------------------------------------------------
  fs04_rebase_c6cf484.py  wandelt fs04_probe_patch.py von Fassung 2 auf 3:

      python3 method/fs04_rebase_c6cf484.py method/fs04_probe_patch.py

  Genau einmal laufen lassen — ein zweiter Lauf bricht mit einer Assertion
  ab, weil die Anker dann schon ersetzt sind. Das ist Absicht.

  Damit aufgenommen:
    ../raw/g08-FS04-rebased.TXT   (der Kartenbericht, injektorgetrieben)

  Was sich aendert:
   * Die EMMC-Instrumentierung haengt an den NEUEN Funktionen. Kein
     Funktionsrumpf wird nachgebaut: jede Wartefunktion behaelt ihren Rumpf
     (`*_inner`) und bekommt nur einen duennen Zeitmantel davor. Eine spaetere
     Aenderung im Rumpf kann dieser Patch also nicht still zuruecknehmen.
   * Der Scope-Fehler der Sonde ist behoben. Vorher sass die Buchung hinter
     dem `?` — eine Wartezeit, die ihr Budget ausschoepfte, wurde gar nicht
     gebucht. Deshalb standen in ../raw/pi04-FS04.TXT `dev.write_one n=11`
     gegen `dev.w.wait_dat n=23`. Jetzt bucht jeder Mantel auf JEDEM Ausgang,
     und zwei `scope.check.*`-Zeilen drucken die Identitaet, statt sie zu
     behaupten.
   * Die eigenen Zaehler des Treibers stehen im Block: `emmc.errors`,
     `emmc.timeouts_by_phase`, `emmc.error_bits`, `emmc.last`, je mit dem
     Zuwachs dieses Blocks.
   * `emmc.spin` faellt weg; eine NOTE-Zeile im Block sagt warum.

  Alles andere unveraendert: gleiches Blockformat, gleiche Datei
  C:\FS04.TXT, gleiches Anhaengen, gleiche Reihenfolgegarantie.

  KEIN `.diff` fuer diese Fassung: der Scratch-Datentraeger lief voll, bevor
  er erzeugt werden konnte (siehe ../pi-probe/STALE-DO-NOT-FLASH.TXT). Das
  Skript oben ist der vollstaendige Ersatz.


UEBRIGE SKRIPTE (fuer alle Fassungen)
-------------------------------------
  make-sd-image-4part.sh  das Vier-Partitionen-Kartenabbild (C:-F:)
  seed-bigfile.sh         C:\BIG1M.BIN (1 MiB) dazulegen
  make_scratch_fs04.sh    Scratch-Kopie + Injektorkette + Sonde
  fs04_mark_hooks.py      MARK RESET/REPORT fuer den Injektor
  fs04_redraw_bench.py    Neuzeichen-Messreihe + erzwungene Geometrie
  run-qemu.sh             ein QEMU-Lauf mit Zeitgrenze (macOS hat kein timeout)
  fs04-probe.actions      Sondierungslauf fuer die Klickpunkte
  fs04-gesture.actions    die Zieh-Geste

Ein Lauf wird so nachgestellt:

  ./make-sd-image-4part.sh <repo> <out.img> <tmpdir>
  ./seed-bigfile.sh <out.img>                       # nur fuer den 1-MiB-Fall
  rsync die Crates in eine Scratch-Kopie, dann
  python3 fs04_probe_patch.py <scratch> gesture|driver
  cargo run -p xtask --release -- build --board rpi4 --release
  ./run-qemu.sh <scratch>/kernel8.img <out.img> <prefix> <sekunden>

ACHTUNG Platzbedarf: jede Scratch-Kopie traegt ein eigenes `target/`
(~2-3 GB) und jedes Kartenabbild 2 GiB. Diese Runde ist genau daran
gescheitert — vor einem Lauf den Platz auf dem Scratch-Datentraeger pruefen
und alte `out/*.img` und Scratch-Baeume wegraeumen.
