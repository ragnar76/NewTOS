#!/usr/bin/env bash
# cards2.sh -- additional base cards of this run (../mkcard.sh, private 64 MiB FAT16, label P4FPRB, never a real card):
#   dcm  NEWDESK (desk48.inf windows W1 C:\, W2 C:\, W3 C:\SUB\): README.TXT, LONGNAME.TXT (12 chars), SUB\NOTE.TXT, SUB\OTHER.TXT,
#        FOLD\A.TXT FOLD\B.TXT FOLD\INNER\C.TXT (folder with contents), BIG.TXT (40 KiB, several clusters)
#   tcp  TeraDesk #Z (as tdd) + the same data tree (copy test)
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
T="$WS/sdk/apps/teradesk/build/dist"
DIRS="SUB FOLD" bash "$HERE/../mkcard.sh" "$C/dcm.img" "$HERE/desk48.inf" "${D[@]}" > "$C/dcm.log" 2>&1
DIRS="" bash "$HERE/../mkcard.sh" "$C/tcp.img" "$HERE/z-teradesk.inf" "${D[@]}" \
  Apps/TERADESK/DESKTOP.NAP="$T/DESKTOP.NAP" Apps/TERADESK/desktop.rsc="$T/desktop.rsc" Apps/TERADESK/icons.rsc="$T/icons.rsc" \
  Apps/TERADESK/cicons.rsc="$T/cicons.rsc" Apps/TERADESK/TERADESK.INF="$HERE/TERADESK.INF" > "$C/tcp.log" 2>&1
shasum -a 256 "$C/dcm.img" "$C/tcp.img"
