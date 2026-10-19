#!/usr/bin/env bash
# misc2.sh <side> -- Doom start (45 s, dumps t30/t45), CoreMark (AUTO/CORE.NCP, ends on the CoreMark result), zView with an image
# argument (JPG/PNG/BMP, dumps t40/t80) -- each on a per-run clone, memprot default (on); then fsck + listing.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SIDE="${1:?}"; K="$S/bin/kernel8-$SIDE-plain.img"; C="$S/cards"
export FSCK=1 CARDLS=1
LSDIRS="/ /DOOM" bash "$HERE/regress/run.sh" "$K" "$C/doom.img" "mi-$SIDE-doom" "serial=on" 'NEVER-ENDS' 50 30 45
LSDIRS="/ /AUTO" bash "$HERE/regress/run.sh" "$K" "$C/core.img" "mi-$SIDE-core" "serial=on" 'CoreMark 1.0 : [0-9]' 400
for n in T-JPG TRGB-PNG T24-BMP; do
  LSDIRS="/" bash "$HERE/regress/run.sh" "$K" "$C/zv-$n.img" "mi-$SIDE-zv-$n" "serial=on" 'NEVER-ENDS' 80 40 75
done
