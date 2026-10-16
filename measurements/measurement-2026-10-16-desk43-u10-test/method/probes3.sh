#!/usr/bin/env bash
# probes3.sh -- reduced SDK probe set on the product kernel (EL0 NEWDESK): accfree, e9mem, stdoutdb, ttya, modes on off strict (recipes of probes2.sh; cards seeded by seed-card.sh as in mem05 acc-series.sh)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards; K="$HERE/../bin/kernel8-u10t.img"
for m in on off strict; do
  M="$m"; . "$HERE/mode.sh"; T="p-el0-$m"
  rd() { APPEND="$A4" bash "$HERE/run-rpi4-drive.sh" "$@"; }
  rd "$K" "$C/accfree.img" "$T-rpi4-accfree" --end 'ACCFREE: (PASS|FAIL)' --end-delay 2 --timeout 400
  rd "$K" "$C/e9mem.img" "$T-rpi4-e9mem" --end '\[E9MEM\] E9MEM: (PASS|FAIL)' --end-delay 2 --timeout 400
  rd "$K" "$C/stdoutdb.img" "$T-rpi4-stdoutdb" --end 'DBG: DONE' --end-delay 2 --timeout 200
  rd "$K" "$C/ttya.img" "$T-rpi4-ttya" --end 'TTYPROBE: (PASS|FAIL)' --end-delay 2 --timeout 200
done
