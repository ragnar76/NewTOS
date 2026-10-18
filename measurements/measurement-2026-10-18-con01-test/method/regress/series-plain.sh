#!/usr/bin/env bash
# series-plain.sh <side> <mode...> -- APITEST, S2ALL (on/off/verify) or S2ALLS (strict, s2s kernel), VFORKT per memprot mode
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/../env.sh"; SIDE="${1:?wt|head}"; shift; B="$S/bin"; C="$S/cards"
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; verify) A="serial=on memprot=verify";; esac
  bash "$HERE/run.sh" "$B/kernel8-$SIDE-plain.img" "$C/api.img" "rg-$SIDE-$m-api" "$A" 'TOTAL ' 240
  if [ $m = strict ]; then
    bash "$HERE/run.sh" "$B/kernel8-$SIDE-s2s.img" "$C/s2s.img" "rg-$SIDE-$m-s2alls" "$A" 'S2ALL: (PASS|FAIL)' 300
  else
    bash "$HERE/run.sh" "$B/kernel8-$SIDE-plain.img" "$C/s2.img" "rg-$SIDE-$m-s2all" "$A" 'S2ALL: (PASS|FAIL)' 300
  fi
  bash "$HERE/run.sh" "$B/kernel8-$SIDE-plain.img" "$C/vf.img" "rg-$SIDE-$m-vforkt" "$A" 'VFORK: (PASS|FAIL)' 240
done
