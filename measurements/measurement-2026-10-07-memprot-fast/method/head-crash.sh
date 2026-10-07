#!/usr/bin/env bash
# head-crash.sh <crash kernel> <cards dir> <tag prefix> <mode> -- MEMPROT-FAST: the crash-kernel
# runs of runs-e9.sh (CRASHTST, UACC3A, MPROT, E10) alone, for the HEAD
# baseline crash kernel (git archive HEAD + the same instrumentation).
set -uo pipefail
K="$1"; C="$2"; PFX="$3"; M="$4"   # not P: mode.sh sets P
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/mode.sh"
export APPEND="${APPEND_OVERRIDE:-$A4}"
T="$PFX-$M"
bash "$HERE/run-rpi4.sh" "$K" "$C/crash.img" "$T-rpi4-crashtst" 40:t40.png 70:t70.png
bash "$HERE/run-rpi4-drive.sh" "$K" "$C/uacc.img" "$T-rpi4-uacc3a" --end 'UACC3A: (PASS|FAIL)' --end-delay 8 --timeout 600
bash "$HERE/run-rpi4-drive.sh" "$K" "$C/mprot-$M.img" "$T-rpi4-mprot" --end 'MPROT3B: (PASS|FAIL)' --end-delay 8 --timeout 300
bash "$HERE/run-rpi4-drive.sh" "$K" "$C/e10-$M.img" "$T-rpi4-e10" --end 'E10USR: (PASS|FAIL)' --end-delay 3 --timeout 300
