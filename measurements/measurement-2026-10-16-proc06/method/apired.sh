#!/usr/bin/env bash
# apired.sh -- PROC06 control: the new APITEST card (apitest-p06.img, case T33) on the PRE-PROC06 product kernel
# measurements/measurement-2026-10-16-t16-desk60/bin/kernel8-t16d60.img (the shared tree before this change; image gitignored, sha256 in that
# run's bin/SHA256SUMS), mode on, EL0 default. T33 goes through the Pterm trap, which masked before PROC06 too, so T33 is expected to pass
# here as well: it pins the Pterm path; the path PROC06 changes (Ctrl-C) is ctrlc.sh's.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; K="$HERE/../../measurement-2026-10-16-t16-desk60/bin/kernel8-t16d60.img"
M=on; . "$HERE/mode.sh"
bash "$HERE/mrun.sh" "$K" /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06/cards/apitest-p06.img "apired-el0-on-apitest" 'TOTAL +[0-9]+@1=2:t-a.png'
