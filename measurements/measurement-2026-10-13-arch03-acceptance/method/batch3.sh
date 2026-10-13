#!/usr/bin/env bash
# batch3.sh <label> <cards-root> -- A13 reruns of two strict runs that the first series lost to a host stall
# (e9plain: no boot progress after "[proc] stand-in PDs" and a 494 s driver timeout; crashtst: QMP timer
# fired at 954 s). Same scripts/cards/arguments as runs-e9.sh, tags with suffix -rerun.
set -uo pipefail
L="$1"; C="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
M=strict; . "$HERE/mode.sh"; T="$L-strict"
for n in 1 2; do
bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-$L.img" "$C/strict/e9plain.img" "$T-rpi4-e9plain-rerun$n" --end 'E9PLAIN: (PASS|FAIL)' --end-delay 2 --timeout 240
done
bash "$HERE/run-rpi4.sh" "$B/kernel8-crash-$L.img" "$C/strict/crash.img" "$T-rpi4-crashtst-rerun" 40:t40.png 70:t70.png
echo "=== batch3 done $(date)"
