#!/usr/bin/env bash
# apitest2.sh -- LIBC-FSTAT: the series APITEST runs again (on/off/strict, EL0) with the corrected apitest card (cards.sh: + C:\PTERMCH.NCP).
# The first pass (card without PTERMCH: T33 SKIP x3, TOTAL 106/0/8; off stalled after BC28 under host load) is kept as first-s-el0-*.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/lf/cards; K="$HERE/../bin/kernel8-lf.img"
for m in on off strict; do
  M="$m"; . "$HERE/mode.sh"
  bash "$HERE/mrun.sh" "$K" "$C/apitest.img" "s-el0-$m-apitest" 'TOTAL +[0-9]+@1=2:t-a.png' 'TOTAL +[0-9]+@1=15:t-b.png'
done
