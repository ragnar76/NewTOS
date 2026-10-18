#!/usr/bin/env bash
# direct-series.sh <side> -- a #Z program that crashes ITSELF (CRCHILD bare = case w) / a #Z program ended by Ctrl-C (WAITKEY): back to the desktop?
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?}"; K="$S/bin/kernel8-$SIDE-crash2.img"; C="$S/cards"
for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$K" "$C/dcrash.img" "dr-$SIDE-$m-dcrash" "$A" 'desktop up' 200
  bash "$HERE/regress/run.sh" "$K" "$C/dwait.img"  "dr-$SIDE-$m-dwait"  "$A" 'desktop up' 200
done
