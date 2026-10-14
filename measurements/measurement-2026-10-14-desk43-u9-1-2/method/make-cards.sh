#!/usr/bin/env bash
# make-cards.sh -- DESK43 U9-1/U9-2: the cards of this run, in /tmp/m5j/u912/cards (all from the MEM05 plain card
# /tmp/m5j/u5b/cards/plain.img via derive-card.sh, except cycnd/hg-z (seed-u6a.sh) and cychg (U9-0's card-f1.img)).
#   el1reg/el0reg  `#z`/`#Z 01 NEWDESK@`            el1nap/el0nap  `#z`/`#Z 01 C:\NEWDESK.NAP@` + NEWDESK.NAP (rpi4)
#   el1big/el0big  `#z`/`#Z 01 NEWDESK@` + 150 files F001.TXT..F150.TXT in C:\
#   el0st          `#Z 01 C:\NEWDESK.NAP@` + NDSELFT.NAP as NEWDESK.NAP   el0panic  same with NDPANIC.NAP
#   cycnd          fresh card: NEWDESK.NAP + newdesk-cycnd.inf (#Z NEWDESK.NAP, F1 = C:\NEWDESK.NAP)
#   hg-z           fresh card: HELLOGEM.NAP (wrapper build) + newdesk-z.inf (U9-0)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; W=/tmp/m5j/u912; C="$W/cards"; P=/tmp/m5j/u5b/cards/plain.img
mkdir -p "$C" "$W/nap-rpi4" "$W/bigfiles" "$W/nap-st-n" "$W/nap-panic-n"
cp "$B/NEWDESK-rpi4.NAP" "$W/nap-rpi4/NEWDESK.NAP"; cp "$B/NDSELFT.NAP" "$W/nap-st-n/NEWDESK.NAP"; cp "$B/NDPANIC.NAP" "$W/nap-panic-n/NEWDESK.NAP"
for i in $(seq -w 1 150); do printf 'DESK43 U9-1 big-directory file %s\r\n' $i > "$W/bigfiles/F$i.TXT"; done
d() { bash "$HERE/derive-card.sh" "$P" "$C/$1.img" "${@:2}"; }
d el1reg '#z 01 NEWDESK@'; d el0reg '#Z 01 NEWDESK@'
d el1nap '#z 01 C:\NEWDESK.NAP@' "$W/nap-rpi4/NEWDESK.NAP"; d el0nap '#Z 01 C:\NEWDESK.NAP@' "$W/nap-rpi4/NEWDESK.NAP"
d el1big '#z 01 NEWDESK@' "$W"/bigfiles/*.TXT; d el0big '#Z 01 NEWDESK@' "$W"/bigfiles/*.TXT
d el0st '#Z 01 C:\NEWDESK.NAP@' "$W/nap-st-n/NEWDESK.NAP"; d el0panic '#Z 01 C:\NEWDESK.NAP@' "$W/nap-panic-n/NEWDESK.NAP"
bash "$HERE/seed-u6a.sh" "$C/cycnd.img" "$HERE/newdesk-cycnd.inf" "$W/nap-rpi4/NEWDESK.NAP"
bash "$HERE/seed-u6a.sh" "$C/hg-z.img" "$HERE/newdesk-z.inf" "$B/HELLOGEM.NAP"
cp /tmp/m5j/u90/card-f1.img "$C/cychg.img"
