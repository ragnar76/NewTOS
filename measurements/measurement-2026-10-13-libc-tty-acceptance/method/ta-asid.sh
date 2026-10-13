#!/usr/bin/env bash
# ta-asid.sh -- on-asid (asid=on, memprot on): TTYA, APITEST, desktop on the product kernel (raspi4b).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; K="$B/kernel8-tta.img"
CR=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/0c54029c-ee4e-40c3-bb6a-48ecd5c51e63/scratchpad/cards-ta/on-asid
mkdir -p "$CR"; M=on-asid; . "$HERE/mode.sh"; export APPEND="$A4"; echo "APPEND=$APPEND"
bash "$HERE/seed-card.sh" "$CR/ttya.img" 'C:\TTYA.NAP' > /dev/null
bash "$HERE/seed-card.sh" "$CR/apitest.img" 'C:\APITEST.NCP' > /dev/null
bash "$HERE/seed-card.sh" "$CR/plain.img" - > /dev/null
bash "$HERE/run-rpi4-drive.sh" "$K" "$CR/ttya.img" "tt-on-asid-rpi4-ttya" --end 'TTYPROBE: (PASS|FAIL)' --end-delay 2 --timeout 200
bash "$HERE/run-rpi4.sh" "$K" "$CR/plain.img" "tt-on-asid-rpi4-desktop" 25:t25.png
bash "$HERE/run-rpi4.sh" "$K" "$CR/apitest.img" "tt-on-asid-rpi4-apitest" 25:t25.png 40:t40.png
echo ASID DONE
