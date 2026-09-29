SCHED01 -- was in diesem Verzeichnis liegt und was von woanders kommt
=====================================================================

Hier (neu fuer SCHED01):

  seed.sh                   frische Karte via tools/make-sd-image.sh + die
                            NEWDESK.INF des jeweiligen Laufs (+ Ball-Fixture)
  newdesk-ball.inf          #Z 01 C:\BALL.NAP           (von PORT07 uebernommen)
  newdesk-teradesk.inf      #Z 01 C:\Apps\TERADESK\DESKTOP.NAP
  newdesk-qed.inf           #Z 01 C:\Apps\QED\QED.NAP
  run-rpi4.sh               QEMU raspi4b, seriell in Datei, QMP-Bildabzuege
  run-virt.sh               dasselbe fuer -M virt (Maschinenzeile wie
                            `xtask run --board qemu-virt`)
  qmp_timed.py              QMP-Treiber (Kopie aus measurement-2026-09-28-port07)
  pull-log.sh               C:\<datei> von der Karte holen
  sched01-scaffolding.diff  das Messgeruest der Ball-/TeraDesk-Laeufe

Uebernommen, unveraendert aufgerufen:

  measurement-2026-09-25-acc02-acc04/method/
      make_scratch.sh  acc2.rs  acc2.actions  run.sh  qmp_multi.py  seed.sh
  -- das Klickskript der ACC02/ACC03/ACC04-Abnahme (Beweis 3 und der
     Doppelklick-Start in Beweis 4).  Es baut eine rsync-Kopie der Crates im
     Job-Scratch und patcht NUR diese Kopie, nie den gemeinsamen Baum.

  measurement-2026-09-29-port08/round2-teradesk/method/
      port08-round2-scaffolding.diff
  -- Grundlage von sched01-scaffolding.diff (zwei Zeilen geaendert, siehe
     dessen Kopf).

Die Laufkommandos in der Reihenfolge der Messung:

  bash method/seed.sh   <img> method/newdesk-ball.inf ball
  bash method/run-rpi4.sh <kernel8> <img> <out> ball-fix 45:...png 75:...png 100:...png
  bash method/seed.sh   <img> method/newdesk-teradesk.inf
  bash method/run-rpi4.sh <kernel8> <img> <out> tera-fix 60:...png 75:...png 115:...png
  bash ../measurement-2026-09-25-acc02-acc04/method/make_scratch.sh <tree> <.../acc2.rs>
  bash ../measurement-2026-09-25-acc02-acc04/method/run.sh <tree>/kernel8.img <img> <out> acc2-fix 240 A_IDLE:...png ...
  bash method/seed.sh   <img> method/newdesk-qed.inf
  bash method/run-rpi4.sh <kernel8> <img> <out> qed-fix 45:...png 70:...png
  bash method/run-virt.sh <kernel.img> <out> boot-fix-virt 30:...png 45:...png
