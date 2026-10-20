#!/usr/bin/env bash
# series.sh -- the runs of this measurement in the order they were made (after build-kernels.sh, make-old.sh, build-probe.sh
# for both libc.a, cards.sh). One QEMU at a time.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
bash "$HERE/run-probe.sh" new on strict off
bash "$HERE/run-probe.sh" old on
for v in new old; do
  LSDIRS="/ /Apps/QED" MTYPE="/Apps/QED/DATED.TXT" DL=1.0 bash "$HERE/drive.sh" "$S/bin/kernel8-qfi.img" "$S/cards/qfi-$v.img" qfi-$v inj up:QED="$HERE/qfi.script"
done
for v in new old; do
  LSDIRS="/" DL=1.0 bash "$HERE/drive.sh" "$S/bin/kernel8-tdv.img" "$S/cards/td-$v.img" td-$v inj up:DESKTOP="$HERE/tdv.script"
done
