#!/usr/bin/env bash
# runall.sh <side...> -- the interactive delete/save runs per kernel side (head | wt), one QEMU at a time, marker-keyed dumps
# (drive.sh, DL 0.8 s), fsck + card listing of each clone (run.sh): dsk (NEWDESK, two deletes via the Trash), tdd (TeraDesk,
# delete via its TRASH), qsv (QED: save as C:\APPS\QED\QS.TXT, then save over it).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for side in "$@"; do
  DL=0.8 TMO=150 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-dsk.img" "$S/cards/dsk.img" "dsk-$side" inj desktop="$HERE/dsk.script"
  DL=0.8 TMO=150 bash "$HERE/drive.sh" "$S/bin/kernel8-$side-tdd.img" "$S/cards/tdd.img" "tdd-$side" inj up:DESKTOP="$HERE/tdd.script"
  LSDIRS="/ /APPS/QED" MTYPE="APPS/QED/QS.TXT APPS/QED/QS.BAK" DL=0.8 TMO=150 \
    bash "$HERE/drive.sh" "$S/bin/kernel8-$side-qsv.img" "$S/cards/qsv.img" "qsv-$side" inj up:QED="$HERE/qsv.script"
done
