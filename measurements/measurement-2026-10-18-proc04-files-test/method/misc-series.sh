#!/usr/bin/env bash
# misc-series.sh <side> -- desktop-only boot, desktop+EmuCON-less memprot=verify boot, Doom smoke (45 s, shots t30/t45), per memprot mode
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?}"; K="$S/bin/kernel8-$SIDE-plain.img"; C="$S/cards"
for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$K" "$C/doom.img" "mi-$SIDE-$m-doom" "$A" 'NEVER-ENDS' 50 30 45
done
