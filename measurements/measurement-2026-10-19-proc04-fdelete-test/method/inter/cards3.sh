#!/usr/bin/env bash
# cards3.sh -- zView / CoreMark cards (../mkcard.sh, private, label P4FPRB); doom.img and con01.img are APFS clones of the
# con01-test scratch cards (DOOM.NAP+DOOM1.WAD #Z card / EmuCON P4Z->P4Z2 card), their sha256 in checks/cards.sha256.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; ZV="$WS/sdk/zview/build/dist/ZVIEW"; SM="$WS/sdk/zview/build/samples/IMAGES"
ex=(ZVIEW/ZVIEW.NAP="$ZV/ZVIEW.NAP" ZVIEW/ZVIEW.RSC="$ZV/ZVIEW.RSC" ZVLAUNCH.NAP="$S/probe/ZVLAUNCH.NAP")
for f in "$ZV"/CODECS/* ; do ex+=(ZVIEW/CODECS/$(basename "$f")="$f"); done
for f in "$ZV"/ICONS/*; do ex+=(ZVIEW/ICONS/$(basename "$f")="$f"); done
for f in T.JPG TRGB.PNG T24.BMP; do ex+=(ZVIEW/SAMPLES/$f="$SM/$f"); done
printf '#Z 01 C:\\ZVLAUNCH.NAP@\n' > "$S/mk2/zv.inf"
for s in T.JPG TRGB.PNG T24.BMP; do
  n=$(echo $s | tr . -); printf 'C:\\ZVIEW\\SAMPLES\\%s\r\n' "$s" > "$S/mk2/ZVARG-$n.TXT"
  DIRS="" bash "$HERE/../mkcard.sh" "$C/zv-$n.img" "$S/mk2/zv.inf" "${ex[@]}" ZVARG.TXT="$S/mk2/ZVARG-$n.TXT" > "$C/zv-$n.log" 2>&1
done
printf '' > /dev/null
DIRS="" bash "$HERE/../mkcard.sh" "$C/core.img" "$HERE/../fd.inf" AUTO/CORE.NCP="$WS/sdk/apps/coremark/build/CORE.NCP" > "$C/core.log" 2>&1
shasum -a 256 "$C"/zv-*.img "$C/core.img" "$C/doom.img" "$C/con01.img" "$S/probe/ZVLAUNCH.NAP" "$WS/sdk/apps/coremark/build/CORE.NCP"
