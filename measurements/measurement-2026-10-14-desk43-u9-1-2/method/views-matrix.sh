#!/usr/bin/env bash
# views-matrix.sh <kernel-name> <mode> <card>... -- DESK43 U9-1/U9-2: the eight U8b/U9-0 views (views.script, fed by
# tools/inject-arm.py from the desktop's up event: `desktop` for the built-in EL1 desktop, `up:NEWDESK` for the EL0
# program), screendumps timed from that event (drive-ev.py; offsets = U9-0 views-run.sh's wall offsets minus its
# arm time, 15.0 s), then 3 s more serial for the stack lines. Cards from /tmp/m5j/u912/cards. Tag <kernel>-<mode>-<card>.
set -uo pipefail
K="${1:?}"; MODE="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$MODE"; . "$HERE/mode.sh"
VIEWS="3.0:1-text,6.0:2-viewmenu,8.5:3-icons,13.5:4-about,20.5:5-prefs,27.5:6-background,34.5:7-config,38.0:8-end"
for c in "$@"; do
  bash "$HERE/run-ev.sh" "$HERE/../bin/kernel8-$K.img" "/tmp/m5j/u912/cards/$c.img" "$K-$MODE-$c" \
    --event '\[u91\] (desktop|up:NEWDESK) armed' --shots "$VIEWS" --tail 3 --timeout 150 | tail -2
done
