#!/usr/bin/env bash
# cards.sh -- the base cards of this run (private 64 MiB FAT16 images in the scratch dir, never a real card; runs clone them):
#   sp.img   C:\AUTO\SDKP.NCP (probe parent), C:\SDKK.NCP (tmpfile child), C:\SUB\A.TXT (11 bytes), C:\SUB\B.TXT,
#            empty C:\TMP (mintlib's TOS temp dir "\tmp"), NEWDESK.INF = sp.inf (no #Z)  -- mkcard.sh
#   api.img  APITEST card (#Z 01 C:\APITEST.NCP@ + PTERMCH/DTACHILD), api-card.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; C="$S/cards"; P="$S/prog"; mkdir -p "$C"
DIRS="SUB TMP" bash "$HERE/mkcard.sh" "$C/sp.img" "$HERE/sp.inf" AUTO/SDKP.NCP="$P/SDKP.NCP" SDKK.NCP="$P/SDKK.NCP" \
  SUB/A.TXT="$HERE/A.TXT" SUB/B.TXT="$HERE/B.TXT" > "$C/sp.log" 2>&1
bash "$HERE/api-card.sh" "$C/api.img" api "$P/APITEST.NCP" "$P/PTERMCH.NCP" "$P/DTACHILD.NCP" > "$C/api.log" 2>&1
cat "$C/sp.log"; tail -2 "$C/api.log"
