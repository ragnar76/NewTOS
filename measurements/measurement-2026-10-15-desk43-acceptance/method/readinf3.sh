#!/usr/bin/env bash
# readinf3.sh <mode> -- Read .INF on the saved card (window in the INF): dumps A window open at start, B after the close click, C after Read INF + Return.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
read -ra N <<< "$(python3 "$HERE/fx.py" "$HERE/ri3.script" 1110 4310 6010)"
bash "$HERE/pair2.sh" kxri3 /tmp/m5j/a15/save/saved-el0-on.img ri3 "${1:-on}" '\[desk\] desktop up@1=2:A-start.png' "\\[kxri3\\] desktop feed@${N[0]}=10:B-closed.png" "\\[kxri3\\] desktop feed@${N[1]}=10:B2-fsel.png" "\\[kxri3\\] desktop feed@${N[2]}=14:C-read.png"
