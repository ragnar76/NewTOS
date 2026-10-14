#!/usr/bin/env bash
# pixel-matrix.sh -- DESK43 U9-2: every view of the EL0 NEWDESK runs against the EL1 built-in desktop on the twin card
# (derive-card.sh: the cards differ only in the INF tag byte `#Z` vs `#z`), same kernel (k2v), plus the EL1 off/strict
# and big-directory pairs, and the instrumented base kernel's views against U9-0's (instrument check). pixdiff.py --boxes.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"
V="1-text 2-viewmenu 3-icons 4-about 5-prefs 6-background 7-config 8-end"
cmpset() { for i in $V; do echo "$1 $i: $(python3 "$HERE/pixdiff.py" "$D/$2-$i.png" "$D/$3-$i.png" --boxes)"; done; }
echo "# instrument check: base tree + U9-1 instruments (k1v) vs U9-0 views (plain card)"
for i in $V; do echo "k1v-plain $i: $(python3 "$HERE/pixdiff.py" "$D/../../measurement-2026-10-14-desk43-u9-0/screendumps/views-u90-rpi4-$i.png" "$D/k1v-plain-$i.png")"; done
echo "# EL0 (NEWDESK via #Z) vs EL1 (built-in desktop), kernel k2v"
for m in on off strict; do cmpset "$m reg" "k2v-$m-el1reg" "k2v-$m-el0reg"; done
for m in on off strict; do cmpset "$m nap" "k2v-$m-el1nap" "k2v-$m-el0nap"; done
cmpset "on big" k2v-on-el1big k2v-on-el0big
echo "# EL1 on the U9-2 tree vs EL1 on the base tree (big card): embedding changes nothing"
cmpset "el1 big k1v/k2v" k1v-on-el1big k2v-on-el1big
