#!/usr/bin/env bash
# verify-extra.sh <label> <cards> -- memprot=verify on the probes the developers did not run under verify: MDSTEP (mdlimit=128), MDACC,
# ACCFREE, MDPROBE (default limit: 8189 held blocks, expensive: full recomputation per sync). cwd = /tmp/m5j/acc.
set -uo pipefail
L="${1:?}"; C="${2:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; K="$B/kernel8-$L.img"
M=verify; . "$HERE/mode.sh"; T="$L-verify"; export APPEND="$A4"
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
APPEND="$A4 mdlimit=128" rd "$K" "$C/mdstep.img" "$T-rpi4-mdstep" --end 'MDSTEP: DONE' --end-delay 3 --timeout 3000 --stamp
APPEND="$A4 mdlimit=128" rd "$K" "$C/mdacc.img" "$T-rpi4-mdacc" --end '\[desk\] desktop up' --end-delay 8 --timeout 900 --stamp
rd "$K" "$C/accfree.img" "$T-rpi4-accfree" --end 'ACCFREE: (PASS|FAIL)' --end-delay 2 --timeout 600
rd "$K" "$C/mdprobe.img" "$T-rpi4-mdprobe" --end 'MDPROBE: DONE' --end-delay 3 --timeout 5000 --stamp
echo "=== verify-extra done $(date)"
