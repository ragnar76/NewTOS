#!/usr/bin/env bash
# asid-runs.sh <label> <cards-dir> <mode> -- the ARCH03-4 ASID probe runs in
# one mode (mode.sh: on|off|strict [-asid|-tiny]):
#   raspi4b, kernel8-asidk-<L> (crash-prompt injector + switch-cost counters):
#     #Z C:\ASIDSTR.NAP (ASIDSTRO for memprot=off) + accessory
#     C:\ACC\ASIDACC.NAC -- parts A-D, host-timestamped (drive.py --stamp);
#     #Z C:\ASIDPERF.NAP (ASIDPRFO) + accessory -- longer loops, counters;
#   raspi4b, product kernel8-<L>: the same ASIDPERF card (uninstrumented
#     wall-clock);
#   virt, kernel-virt-asidk-<L>: ASIDSTR(O) from the RAM registry before the
#     AES (parts A-C), crash prompt answered over serial.
set -uo pipefail
L="$1"; C="$2"; M="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
. "$HERE/mode.sh"
T="$L-$M"
mkdir -p "$C"
if [ "$BASE" = off ]; then DS='C:\ASIDSTRO.NAP'; DP='C:\ASIDPRFO.NAP'; else DS='C:\ASIDSTR.NAP'; DP='C:\ASIDPERF.NAP'; fi
[ -f "$C/asidstr.img" ] || EXTRA="ACC/ASIDACC.NAC=$B/ASIDACC.NAC" bash "$HERE/seed-card.sh" "$C/asidstr.img" "$DS" > /dev/null
[ -f "$C/asidperf.img" ] || EXTRA="ACC/ASIDACC.NAC=$B/ASIDACC.NAC" bash "$HERE/seed-card.sh" "$C/asidperf.img" "$DP" > /dev/null
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
rd "$B/kernel8-asidk-$L.img" "$C/asidstr.img" "$T-rpi4-asidstr" --end 'ASIDSTR: (PASS|FAIL)' --end-delay 3 --timeout 500 --stamp
rd "$B/kernel8-asidk-$L.img" "$C/asidperf.img" "$T-rpi4-asidperf-counters" --end 'ASIDPERF: (PASS|FAIL)' --end-delay 3 --timeout 600 --stamp
rd "$B/kernel8-$L.img" "$C/asidperf.img" "$T-rpi4-asidperf" --end 'ASIDPERF: (PASS|FAIL)' --end-delay 3 --timeout 600 --stamp
APPEND="$AV" bash "$HERE/run-virt.sh" "$B/kernel-virt-asidk-$L.img" "$T-virt-asidstr" --answer --end 'ASIDSTR: (PASS|FAIL)' --end-delay 2 --timeout 400 --stamp
