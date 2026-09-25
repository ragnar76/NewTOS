FS04 — method/: zwei Fassungen der Sonde, beide vollstaendig abgelegt
=====================================================================

Es gibt **zwei** Fassungen der Messsonde. Beide sind hier als vollstaendiger
Patch abgelegt; ein `.diff` ist ein kompletter Patch und braucht das
Erzeugerskript nicht.

FASSUNG 1 — nur seriell
-----------------------
  fs04-probe-640.diff     Treiberbetrieb, Faelle A-G, 640x480
  fs04-probe-1920.diff    dasselbe mit erzwungener 1920x1080-Geometrie
  fs04-gesture.diff       Gestenbetrieb + IKBD-Injektor

  Damit wurden aufgenommen:
    ../raw/r03-driver-640x480.log
    ../raw/r04-driver-1920x1080.log
    ../raw/g05-gesture-640x480.log
    ../raw/p01-probe-clickpoints.log
  Die Zahlen in ../RUNINFO.TXT und ../NUMBERS.TXT stammen aus dieser Fassung.

FASSUNG 2 — Bericht zusaetzlich in eine Datei auf der Karte
-----------------------------------------------------------
  fs04_probe_patch.py     das Erzeugerskript (Stand: Fassung 2)
  fs04-pi-handover.diff   Gestenbetrieb OHNE Injektor
                          == genau ../pi-probe/kernel8-fs04probe.img
  fs04-gesture-r2.diff    Gestenbetrieb MIT Injektor (Pruefung in QEMU)

  Damit wurden aufgenommen:
    ../raw/pi03-boot-of-handover-image.log
    ../raw/g07-gesture-report-to-card.log
    ../raw/g07-FS04.TXT        (der Kartenbericht im Wortlaut)

  Neu gegenueber Fassung 1:
    * der Bericht geht nach C:\FS04.TXT, angehaengt, nicht ueberschrieben
    * ein Startblock mit Bildschirmgeometrie/Zaehlerfrequenz/Laufwerken
    * Fseek als `dos_seek` in desk/dosbind.rs (fuers Anhaengen)
    * `vdi_backend::fs04_fb_geometry()` fuer die Kopfzeile
    * reine Zaehlslots werden als `n=<zahl>` gedruckt statt mit sinnlosem
      min/max, und der Block traegt einen Hinweis, dass desk.fun_op /
      desk.copy_pass / desk.fun_drag die Wartezeit des Bestaetigungsdialogs
      mitenthalten und `wall` die Zahl ist, auf die es ankommt

  Die Zahlen aendern sich dadurch nicht messbar (Vergleich g05 gegen g07:
  wall 25 021 us gegen 24 987 us, 6+6 Sektorzugriffe in beiden). Der einzige
  systematische Unterschied: C:\FS04.TXT ist ein Symbol mehr im C:-Fenster,
  was beim Neuzeichnen rund 1 % kostet (fb.clean 2282 gegen 2310).


UEBRIGE SKRIPTE (fuer beide Fassungen)
--------------------------------------
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
