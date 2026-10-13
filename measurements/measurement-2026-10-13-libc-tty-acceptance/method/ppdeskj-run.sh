#!/usr/bin/env bash
# ppdeskj-run.sh <label> <scratch-root> <mode> -- batch2.sh part (c): PPDESKJ
# (bin/j, libc-joined PATH accepted) started by the desktop double-click on the
# ppdesk injector kernel (bin/kernel8-ppdesk-<label>.img from ppdesk-run.sh).
set -uo pipefail
L="$1"; R="$2"; M="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; . "$HERE/mode.sh"; T="$L-$M"
mkdir -p "$R/cards"
[ -f "$R/cards/ppdeskj.img" ] || INF="$HERE/ppdesk/newdesk-ppdesk.inf" EXTRA="PPD/PPDESKJ.NAP=$B/j/PPDESKJ.NAP" \
  bash "$HERE/seed-card.sh" "$R/cards/ppdeskj.img" - > /dev/null
cp "$R/cards/ppdeskj.img" "$R/cards/run-ppdeskj-$M.img"
bash "$HERE/run-marks.sh" "$B/kernel8-ppdesk-$L.img" "$R/cards/run-ppdeskj-$M.img" "$HERE/../screendumps" "$T-rpi4-ppdeskj" 150 \
  "MARK A_IDLE:$T-ppdeskj-A_IDLE.png:0.5" "MARK B_BACK:$T-ppdeskj-B_BACK.png:0.5" || true
mv "$HERE/../screendumps/$T-rpi4-ppdeskj-serial.log" "$HERE/../serial/" 2>/dev/null || true
