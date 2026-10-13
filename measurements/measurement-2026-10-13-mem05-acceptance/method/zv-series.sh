#!/usr/bin/env bash
# zv-series.sh <label> <cards-root> <mode> -- MEM05 acceptance: items the developers only linked, now STARTED:
#   zView (ZVLAUNCH -> ZVIEW.NAP with an image argument, ten LDG codecs), LDGTEST (AUTO, 69 checks), LDGDEMO (client + libs),
#   PRESENT. cwd = /tmp/m5j/acc. Screendumps via QMP; serial verdicts where the program prints one.
set -uo pipefail
L="${1:?}"; C="${2:?}"; m="${3:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; K="$B/kernel8-$L.img"
M="$m"; . "$HERE/mode.sh"; T="$L-$m"; export APPEND="$A4"
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
echo "=== $T zv $(date) APPEND=$A4"
rd "$K" "$C/ldgauto.img" "$T-rpi4-ldgauto" --end 'ldgtest: (ALL PASS|FAIL \()' --end-delay 2 --timeout 400
r4 "$K" "$C/ldgdemo.img" "$T-rpi4-ldgdemo" 30:t30.png 50:t50.png
r4 "$K" "$C/zv-T-JPG.img" "$T-rpi4-zview-jpg" 40:t40.png 80:t80.png
r4 "$K" "$C/zv-TRGB-PNG.img" "$T-rpi4-zview-png" 40:t40.png 80:t80.png
r4 "$K" "$C/zv-T24-BMP.img" "$T-rpi4-zview-bmp" 80:t80.png
r4 "$K" "$C/zv-plain.img" "$T-rpi4-zview-browser" 40:t40.png 80:t80.png
r4 "$K" "$C/present.img" "$T-rpi4-present" 30:t30.png 45:t45.png
echo "=== $T zv done $(date)"
