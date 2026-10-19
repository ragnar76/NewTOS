#!/usr/bin/env bash
# runall.sh <side...> -- the interactive rename/move/save runs per kernel side (head | wt), one QEMU at a time, marker-keyed dumps
# (drive.sh, DL 0.8 s), fsck + card listing of each clone (run.sh): drn (NEWDESK Ctrl-drag move + Show-info rename), trn (TeraDesk
# Show-info rename), qsv (QED: save as C:\APPS\QED\QS.PRG, then save over it -> backup rename QS.PRG -> QS.BAK).
# (Adapted from measurement-2026-10-19-proc04-fdelete-test/method/inter/runall.sh.)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for side in "$@"; do
  LSDIRS="/ /SUB" MTYPE="RENAMED.TXT SUB/LONGNAME.TXT" DL=0.8 TMO=150 \
    bash "$HERE/drive.sh" "$S/bin/kernel8-$side-drn.img" "$S/cards/drn.img" "drn-$side" inj desktop="$HERE/drn.script"
  LSDIRS="/" MTYPE="RENAMED.TXT" DL=0.8 TMO=150 \
    bash "$HERE/drive.sh" "$S/bin/kernel8-$side-trn.img" "$S/cards/trn.img" "trn-$side" inj up:DESKTOP="$HERE/trn.script"
  LSDIRS="/ /APPS/QED" MTYPE="APPS/QED/QS.PRG APPS/QED/QS.BAK" DL=0.8 TMO=150 \
    bash "$HERE/drive.sh" "$S/bin/kernel8-$side-qsv.img" "$S/cards/qsv.img" "qsv-$side" inj up:QED="$HERE/qsv.script"
done
