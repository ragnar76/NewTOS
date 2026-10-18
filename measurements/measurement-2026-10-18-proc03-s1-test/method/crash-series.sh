#!/usr/bin/env bash
# crash-series.sh <side> -- program-exit paths on kernel8-<side>-crash (inject-arm: space.script answers the crash prompts,
# ctrlc.script = Ctrl-C): CRASHTST, UACC3A, CTRLCPAR (Ctrl-C), MPROT3B/S/O, E10USR/S/O per memprot mode; verdict + "[desk] desktop up" afterwards.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?}"; K="$S/bin/kernel8-$SIDE-crash.img"; C="$S/cards"
for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$K" "$C/crashtst.img" "cr-$SIDE-$m-crashtst" "$A" 'desktop up' 400
  bash "$HERE/regress/run.sh" "$K" "$C/uacc.img"     "cr-$SIDE-$m-uacc3a"   "$A" 'desktop up' 600
  bash "$HERE/regress/run.sh" "$K" "$C/ctrlc.img"    "cr-$SIDE-$m-ctrlc"    "$A" 'desktop up' 120
  bash "$HERE/regress/run.sh" "$K" "$C/mprot-$m.img" "cr-$SIDE-$m-mprot"    "$A" 'desktop up' 400
  bash "$HERE/regress/run.sh" "$K" "$C/e10-$m.img"   "cr-$SIDE-$m-e10"      "$A" 'desktop up' 400
done
