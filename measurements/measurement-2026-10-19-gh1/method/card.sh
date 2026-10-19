#!/usr/bin/env bash
# card.sh -- base card: NEWDESK.INF = gh1.inf (C:\ACC\ window open), C:\ACC\SYSACC.NAC built from HEAD sdk/examples/sysacc.c.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; mkdir -p "$S/cards"
bash "$HERE/mkcard.sh" "$S/cards/gh1.img" "$HERE/gh1.inf" ACC/SYSACC.NAC="$S/bin/SYSACC.NAC"
