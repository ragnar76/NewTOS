#!/usr/bin/env bash
# cards-extra.sh -- probe card, matrix cards (c-desk/acc/td/qed), crash/ctrlc/mprot/e10/uacc/doom cards. Private 64 MiB FAT16 images in $S, never a real card.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; I="$S/img"; mkdir -p "$C" "$I"
bash "$HERE/card.sh" "$S/prog" "$C/probe.img" > "$C/probe.log" 2>&1
bash "$HERE/regress/cards.sh" > "$C/regress-cards.log" 2>&1
M="$HERE/m113"; T="$WS/sdk/apps/teradesk/build/dist"; Q="$WS/sdk/apps/qed/build"
bash "$M/mkcard.sh" "$I/c-desk.img" "$M/desk.inf" > /dev/null
bash "$M/mkcard.sh" "$I/c-acc.img" "$M/desk.inf" ACC/SYSACC.NAC="$WS/measurements/measurement-2026-10-13-libc-tty-acceptance/bin/SYSACC.NAC" > /dev/null
bash "$M/mkcard.sh" "$I/c-td.img" "$M/z-teradesk.inf" Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" Apps/TERADESK/cicons.rsc="$T/cicons.rsc" Apps/TERADESK/TERADESK.INF="$M/TERADESK.INF" > /dev/null
bash "$M/mkcard.sh" "$I/c-qed.img" "$M/z-qed.inf" Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc" > /dev/null
X=""; for f in "$S"/crash/*; do X="$X $(basename "$f")=$f"; done
mk() { printf '#Z 01 C:\\%s@\r\n' "$2" > "$C/$1.inf"; bash "$HERE/mkcard.sh" "$C/$1.img" "$C/$1.inf" $X > "$C/$1.log" 2>&1; }
mk crashtst CRASHTST.NAP; mk ctrlc CTRLCPAR.NCP; mk uacc UACC3A.NAP
mk mprot-on MPROT3B.NAP; mk mprot-strict MPROTS.NAP; mk mprot-off MPROTO.NAP
mk e10-on E10USR.NAP; mk e10-strict E10USRS.NAP; mk e10-off E10USRO.NAP
D="$WS/sdk/apps/doom/build/DOOM.NAP"; W="$WS/Applications/doom-wad/doom1.wad"
printf '#Z 01 C:\\DOOM\\DOOM.NAP@\r\n' > "$C/doom.inf"; bash "$HERE/mkcard.sh" "$C/doom.img" "$C/doom.inf" DOOM/DOOM.NAP="$D" DOOM/DOOM1.WAD="$W" > "$C/doom.log" 2>&1
ls -la "$C" "$I" | grep img
