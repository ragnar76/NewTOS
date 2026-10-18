#!/usr/bin/env bash
# series.sh <side> -- the regression runs of this step on kernel side <side> (wt | head), modes on / strict / off:
# APITEST (#Z, plain kernel), S2ALL (on/off) / S2ALLS (strict, s2s kernel: Space answers its two intended crash prompts),
# VFORKT alone, TeraDesk -> F1 -> QED (tdq kernel; screendumps t25/t40). One QEMU at a time.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/../env.sh"; SIDE="${1:?wt|head}"; B="$S/bin"; C="$S/cards"
for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/run.sh" "$B/kernel8-$SIDE-plain.img" "$C/api.img" "rg-$SIDE-$m-api" "$A" 'TOTAL ' 240
  if [ $m = strict ]; then
    bash "$HERE/run.sh" "$B/kernel8-$SIDE-s2s.img" "$C/s2s.img" "rg-$SIDE-$m-s2alls" "$A" 'S2ALL: (PASS|FAIL)' 300
  else
    bash "$HERE/run.sh" "$B/kernel8-$SIDE-plain.img" "$C/s2.img" "rg-$SIDE-$m-s2all" "$A" 'S2ALL: (PASS|FAIL)' 300
  fi
  bash "$HERE/run.sh" "$B/kernel8-$SIDE-plain.img" "$C/vf.img" "rg-$SIDE-$m-vforkt" "$A" 'VFORK: (PASS|FAIL)' 240
  bash "$HERE/run.sh" "$B/kernel8-$SIDE-tdq.img" "$C/tdq.img" "rg-$SIDE-$m-tdqed" "$A" 'NEVER-ENDS' 45 25 40
done
