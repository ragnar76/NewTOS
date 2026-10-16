#!/usr/bin/env bash
# (PROC06: copied from measurement-2026-10-16-t16-desk60/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06, kernel id p06; changes listed in RUNINFO.TXT.)
# (T16+DESK60: copied from measurement-2026-10-16-desk58/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/t16d60, kernel id t16d60.)
# (DESK58 copy of measurement-2026-10-16-desk59/method (itself a copy of measurement-2026-10-15-desk43-acceptance/method); scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06 instead of a15.)
# series2.sh <mode> [paths] -- acceptance series, marker-keyed: desktop / APITEST / TeraDesk / QED on kernel bin/kernel8-p06.img, EL0 default
# (tag s-el0-<mode>-...) and desk=kernel (s-el1-<mode>-...). Cards /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards. One QEMU at a time.
set -uo pipefail
m="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/u5b/cards; K="$HERE/../bin/kernel8-p06.img"
M="$m"; . "$HERE/mode.sh"; BASEARGS="$APPEND"
for p in ${2:-el0 el1}; do
  if [ $p = el1 ]; then export APPEND="$BASEARGS desk=kernel"; else export APPEND="$BASEARGS"; fi
  T="s-$p-$m"
  bash "$HERE/mrun.sh" "$K" $C/plain.img "$T-desktop" '\[desk\] desktop up@1=8:t.png'
  bash "$HERE/mrun.sh" "$K" "${APICARD:-/Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06/cards/apitest-p06.img}" "$T-apitest" 'TOTAL +[0-9]+@1=2:t-a.png' 'TOTAL +[0-9]+@1=15:t-b.png'
  bash "$HERE/mrun.sh" "$K" $C/teradesk.img "$T-teradesk" '\[launch\] EL0: slot@1=12:t.png'
  bash "$HERE/mrun.sh" "$K" $C/qed.img "$T-qed" '\[launch\] EL0: slot@1=12:t.png'
done
