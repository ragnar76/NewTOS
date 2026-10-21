#!/usr/bin/env bash
# gh1plain.sh <kernel> <tag> -- the developer run's GH1 popup case (gh1plain.script = measurement-2026-10-21-gh1/method/gh1.script)
# on the 2026-10-21-gh1 card (scratch gh1run/cards/gh1.img, sha256 f2864d27...), via gh1run.py (= that run's run.py, unchanged).
# Shots: desk = 1 s after the first move (+200), open = 3 s after the Desk-menu pick (+510), popup = 3 s after the popup press
# (+1610), esc = 3 s after Esc (+3010), closed = 3 s after the Cancel click (+4510). -> ../screendumps-gh1/<tag>-*.png, ../serial/<tag>.log
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
G=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/651de586-7dd7-487b-9e92-c8525c58f508/scratchpad/gh1run
python3 "$HERE/gh1run.py" "$1" "$G/cards/gh1.img" "$2" "$HERE/../serial" "$HERE/../screendumps-gh1" 150 \
  200:1.0:desk 510:3.0:open 1610:3.0:popup 3010:3.0:esc 4510:3.0:closed
rm -f "$G/cards/run-$2/card.img" "$G/cards/run-$2/shot.ppm"; rmdir "$G/cards/run-$2" 2>/dev/null || true
