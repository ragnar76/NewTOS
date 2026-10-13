#!/usr/bin/env bash
# seed-extra.sh <cards-dir> -- MEM05 acceptance: zView (+codecs) / LDG / present cards. Uses seed-card.sh EXTRA.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"; B="$HERE/../bin"; C="${1:?}"
ZV="$WS/sdk/zview/build/dist/ZVIEW"
ex=""
for f in "$ZV"/ZVIEW.NAP "$ZV"/ZVIEW.RSC; do ex="$ex ZVIEW/$(basename $f)=$f"; done
for f in "$ZV"/CODECS/*; do ex="$ex ZVIEW/CODECS/$(basename $f)=$f"; done
for f in "$ZV"/ICONS/*; do ex="$ex ZVIEW/ICONS/$(basename $f)=$f"; done
for f in "$WS"/sdk/zview/build/samples/IMAGES/*; do ex="$ex ZVIEW/SAMPLES/$(basename $f)=$f"; done
ex="$ex ZVLAUNCH.NAP=$B/ZVLAUNCH.NAP"
for s in T.JPG TRGB.PNG T24.BMP; do
  n=$(echo $s | tr . -); rm -f "$C/zv-$n.img"
  # ZVARG.TXT: the argument zView is launched with
  printf 'C:\\ZVIEW\\SAMPLES\\%s\r\n' "$s" > "$C/ZVARG-$n.TXT"
  EXTRA="$ex ZVARG.TXT=$C/ZVARG-$n.TXT" bash "$HERE/seed-card.sh" "$C/zv-$n.img" 'C:\ZVLAUNCH.NAP' > /dev/null
done
# plain zView start (no argument): browser
EXTRA="$ex" bash "$HERE/seed-card.sh" "$C/zv-plain.img" 'C:\ZVIEW\ZVIEW.NAP' > /dev/null
L="$WS/sdk/ldg/build"
EXTRA="LDG/EX_LIB.NLD=$L/EX_LIB.NLD AUTO/LDGTEST.NCP=$L/LDGTEST.NCP" bash "$HERE/seed-card.sh" "$C/ldgauto.img" - > /dev/null
EXTRA="LDGDEMO/CLIENT/LDGDEMO.NAP=$L/client/dist/LDGDEMO/CLIENT/LDGDEMO.NAP LDGDEMO/CLIENT/CLIENT.RSC=$L/client/dist/LDGDEMO/CLIENT/CLIENT.RSC LDGDEMO/LIBS/EX_LIB.NLD=$L/client/dist/LDGDEMO/LIBS/EX_LIB.NLD LDGDEMO/LIBS/POPUP.NLD=$L/client/dist/LDGDEMO/LIBS/POPUP.NLD" bash "$HERE/seed-card.sh" "$C/ldgdemo.img" 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' > /dev/null
P="$WS/sdk/apps/present"
EXTRA="Apps/PRESENT/PRESENT.NAP=$P/build/PRESENT.NAP Apps/PRESENT/SLIDES.TXT=$P/sample/SLIDES.TXT Apps/PRESENT/TEST.PNG=$P/build/TEST.PNG" bash "$HERE/seed-card.sh" "$C/present.img" 'C:\Apps\PRESENT\PRESENT.NAP' > /dev/null
echo seeded-extra
