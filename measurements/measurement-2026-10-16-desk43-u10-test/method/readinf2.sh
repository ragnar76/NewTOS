#!/usr/bin/env bash
# readinf2.sh <mode> -- "Read .INF file" end to end (kernel kxri2, ri2.script), plain card, EL0 and desk=kernel, marker-keyed dumps.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
read -ra N <<< "$(python3 "$HERE/fx.py" "$HERE/ri2.script" 2810 3610 6310 9010)"
bash "$HERE/pair2.sh" kxri2 /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/plain.img ri2 "${1:-on}" "\\[kxri2\\] desktop feed@${N[0]}=8:save.png" "\\[kxri2\\] desktop feed@${N[1]}=8:closed.png" "\\[kxri2\\] desktop feed@${N[2]}=8:fsel.png" "\\[kxri2\\] desktop feed@${N[3]}=10:read.png"
