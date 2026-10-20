#!/usr/bin/env bash
# cards.sh -- the base card of this run (private 64 MiB FAT16 image in the scratch dir, never a real card; runs clone it):
#   rt.img   C:\AUTO\RTP.NCP (the probe, runs before the AES), NEWDESK.INF = rt.inf (no #Z) -- mkcard.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; mkdir -p "$C"
DIRS="" bash "$HERE/mkcard.sh" "$C/rt.img" "$HERE/rt.inf" AUTO/RTP.NCP="$S/prog/RTP.NCP" > "$C/rt.log" 2>&1
cat "$C/rt.log"; shasum -a 256 "$C/rt.img"
