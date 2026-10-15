#!/usr/bin/env bash
# all-d58.sh -- DESK58 (copy of desk59 all-d59.sh): 8 views (kernel kv) and the series (kernel d58 = checks.sh product build), EL0 default desktop, modes on/off/strict.
# One QEMU at a time.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in on off strict; do PATHS=el0 bash "$HERE/views2.sh" $m; done
for m in on off strict; do bash "$HERE/series2.sh" $m el0; done
