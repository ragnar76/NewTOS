#!/usr/bin/env bash
# cards.sh -- base cards of the interactive rename/move/save runs (../mkcard.sh, private 64 MiB FAT16, label FS18PRB, never a real card).
# Data tree as measurement-2026-10-19-proc04-fdelete-test/method/inter/cards2.sh:
#   drn  NEWDESK (desk48.inf windows W1 C:\, W2 C:\, W3 C:\SUB\): README.TXT, LONGNAME.TXT, BIG.TXT, SUB\NOTE.TXT, SUB\OTHER.TXT,
#        FOLD\A.TXT FOLD\B.TXT FOLD\INNER\C.TXT
#   trn  TeraDesk #Z (z-teradesk.inf, TERADESK.INF) + DELME.TXT, KEEP.TXT (the tdd card of that run: same window layout)
#   qsv  QED #Z (z-qed.inf), no data files
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; mkdir -p "$C" "$S/mk2"
M="$S/mk2"
printf 'readme text\r\n' > $M/README.TXT; printf 'long name file\r\n' > $M/LONGNAME.TXT
printf 'note one\r\n' > $M/NOTE.TXT; printf 'other file\r\n' > $M/OTHER.TXT
printf 'file A in FOLD\r\n' > $M/A.TXT; printf 'file B in FOLD, a bit longer\r\n' > $M/B.TXT; printf 'file C in INNER\r\n' > $M/C.TXT
python3 -I -c "
import sys
sys.stdout.buffer.write(b''.join(b'line %05d of the big file, padding padding pad\r\n' % i for i in range(800)))" > $M/BIG.TXT
D=(README.TXT="$M/README.TXT" LONGNAME.TXT="$M/LONGNAME.TXT" BIG.TXT="$M/BIG.TXT" SUB/NOTE.TXT="$M/NOTE.TXT" SUB/OTHER.TXT="$M/OTHER.TXT"
   FOLD/A.TXT="$M/A.TXT" FOLD/B.TXT="$M/B.TXT" FOLD/INNER/C.TXT="$M/C.TXT")
T="$WS/sdk/apps/teradesk/build/dist"; Q="$WS/sdk/apps/qed/build"
DIRS="SUB FOLD" bash "$HERE/../mkcard.sh" "$C/drn.img" "$HERE/desk48.inf" "${D[@]}" > "$C/drn.log" 2>&1
DIRS="" bash "$HERE/../mkcard.sh" "$C/trn.img" "$HERE/z-teradesk.inf" DELME.TXT="$HERE/DELME.TXT" KEEP.TXT="$HERE/KEEP.TXT" \
  Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" \
  Apps/TERADESK/cicons.rsc="$T/cicons.rsc" Apps/TERADESK/TERADESK.INF="$HERE/TERADESK.INF" > "$C/trn.log" 2>&1
DIRS="" bash "$HERE/../mkcard.sh" "$C/qsv.img" "$HERE/z-qed.inf" Apps/QED/QED.NAP="$Q/QED.NAP" Apps/QED/qed.rsc="$Q/qed.rsc" > "$C/qsv.log" 2>&1
shasum -a 256 "$T/DESKTOP.NAP" "$Q/QED.NAP" "$C"/drn.img "$C"/trn.img "$C"/qsv.img
