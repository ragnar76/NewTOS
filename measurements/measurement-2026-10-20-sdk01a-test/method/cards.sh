#!/usr/bin/env bash
# cards.sh -- base cards for the OLD and NEW libc variants (private 64 MiB FAT16 images in the scratch dir, never a real card; runs clone them).
# Variant payloads come from $S/<v>/sdk (v = new|old: working-tree sdk after SDK01 A vs. HEAD sdk files; see RUNINFO), ZVLAUNCH from $S/probe.
#   qed-<v>   QED #Z (z-qed.inf) + Apps/QED/{QED.NAP,qed.rsc}
#   td-<v>    TeraDesk #Z + TERADESK-QED.INF (F1 = C:\APPS\QED\QED.NAP) + Apps/QED + data tree (FOLD, SUB, BIG.TXT, ...)
#   zv-<v>-<X> ZVLAUNCH #Z, ZVIEW dir (NAP + codecs + icons + rsc) + sample images, ZVARG.TXT names the image X (JPG PNG GIF BMP PI1)
#   doom-<v>  DOOM\DOOM.NAP + DOOM1.WAD, #Z
#   core-<v> / coreval-<v>  C:\AUTO\CORE.NCP / COREVAL.NCP (CoreMark performance / validation run)
#   api-<v>   APITEST card
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; mkdir -p "$C" "$S/mk2"; M="$S/mk2"
printf 'readme text\r\n' > $M/README.TXT; printf 'long name file\r\n' > $M/LONGNAME.TXT
printf 'note one\r\n' > $M/NOTE.TXT; printf 'other file\r\n' > $M/OTHER.TXT
printf 'file A in FOLD\r\n' > $M/A.TXT; printf 'file B in FOLD, a bit longer\r\n' > $M/B.TXT; printf 'file C in INNER\r\n' > $M/C.TXT
python3 -I -c "
import sys
sys.stdout.buffer.write(b''.join(b'line %05d of the big file, padding padding pad\r\n' % i for i in range(800)))" > $M/BIG.TXT
D=(README.TXT="$M/README.TXT" LONGNAME.TXT="$M/LONGNAME.TXT" BIG.TXT="$M/BIG.TXT" SUB/NOTE.TXT="$M/NOTE.TXT" SUB/OTHER.TXT="$M/OTHER.TXT"
   FOLD/A.TXT="$M/A.TXT" FOLD/B.TXT="$M/B.TXT" FOLD/INNER/C.TXT="$M/C.TXT")
printf '#Z 01 C:\\ZVLAUNCH.NAP@\n' > "$M/zv.inf"; printf '#Z 01 C:\\DOOM\\DOOM.NAP@\n' > "$M/doom.inf"
for v in new old; do
  SD="$S/$v/sdk"; ZV="$SD/zview/build/dist/ZVIEW"; SM="$WS/sdk/zview/build/samples/IMAGES"; T="$SD/apps/teradesk/build/dist"; Q="$SD/apps/qed/build"
  DIRS="" bash "$HERE/mkcard.sh" "$C/qed-$v.img" "$HERE/z-qed.inf" Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc" > "$C/qed-$v.log" 2>&1
  DIRS="" bash "$HERE/mkcard.sh" "$C/td-$v.img" "$HERE/z-teradesk.inf" "${D[@]}" \
    Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" \
    Apps/TERADESK/cicons.rsc="$T/cicons.rsc" Apps/TERADESK/TERADESK.INF="$HERE/TERADESK-QED.INF" \
    Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc" > "$C/td-$v.log" 2>&1
  ex=(ZVIEW/ZVIEW.NAP="$ZV/ZVIEW.NAP" ZVIEW/ZVIEW.RSC="$ZV/ZVIEW.RSC" ZVLAUNCH.NAP="$S/probe/ZVLAUNCH-$v.NAP")
  for f in "$ZV"/CODECS/*; do ex+=(ZVIEW/CODECS/$(basename "$f")="$f"); done
  for f in "$ZV"/ICONS/*; do ex+=(ZVIEW/ICONS/$(basename "$f")="$f"); done
  for f in T.JPG TRGB.PNG T.GIF T24.BMP T.PI1; do ex+=(ZVIEW/SAMPLES/$f="$SM/$f"); done
  for s in T.JPG TRGB.PNG T.GIF T24.BMP T.PI1; do
    n=${s##*.}; printf 'C:\\ZVIEW\\SAMPLES\\%s\r\n' "$s" > "$M/ZVARG-$n.TXT"
    DIRS="" bash "$HERE/mkcard.sh" "$C/zv-$v-$n.img" "$M/zv.inf" "${ex[@]}" ZVARG.TXT="$M/ZVARG-$n.TXT" > "$C/zv-$v-$n.log" 2>&1
  done
  DIRS="" bash "$HERE/mkcard.sh" "$C/doom-$v.img" "$M/doom.inf" DOOM/DOOM.NAP="$SD/apps/doom/build/DOOM.NAP" DOOM/DOOM1.WAD="$WS/Applications/doom-wad/doom1.wad" > "$C/doom-$v.log" 2>&1
  DIRS="" bash "$HERE/mkcard.sh" "$C/core-$v.img" "$HERE/sp.inf" AUTO/CORE.NCP="$SD/apps/coremark/build/CORE.NCP" > "$C/core-$v.log" 2>&1
  DIRS="" bash "$HERE/mkcard.sh" "$C/coreval-$v.img" "$HERE/sp.inf" AUTO/CORE.NCP="$SD/apps/coremark/build/COREVAL.NCP" > "$C/coreval-$v.log" 2>&1
  bash "$HERE/api-card.sh" "$C/api-$v.img" api "$SD/apps/apitest/APITEST.NCP" "$S/prog-new/PTERMCH.NCP" "$S/prog-new/DTACHILD.NCP" > "$C/api-$v.log" 2>&1
done
DIRS="SUB TMP" bash "$HERE/mkcard.sh" "$C/sp.img" "$HERE/sp.inf" AUTO/SDKP.NCP="$S/prog-new/SDKP.NCP" SDKK.NCP="$S/prog-new/SDKK.NCP" SUB/A.TXT="$HERE/A.TXT" SUB/B.TXT="$HERE/B.TXT" > "$C/sp.log" 2>&1
ls -la "$C"/*.img | awk '{print $5, $9}'
