#!/usr/bin/env bash
# (PROC06: copied from measurement-2026-10-16-t16-desk60/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06, kernel id p06; changes listed in RUNINFO.TXT.)
# all.sh -- T16+DESK60 (copy of measurement-2026-10-16-desk58/method/all-d58.sh): 8 views (kernel kv, built with tools/inject-arm.py) and the series
# (kernel t16d60 = checks.sh product build), EL0 default desktop, modes on/off/strict. One QEMU at a time.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in on off strict; do PATHS=el0 bash "$HERE/views2.sh" $m; done
for m in on off strict; do bash "$HERE/series2.sh" $m el0; done
