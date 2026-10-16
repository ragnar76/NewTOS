#!/usr/bin/env bash
# q-series.sh -- U10 test: series2.sh (EL0 only, the only path left) for all five modes, one QEMU at a time. cwd = scratch (relative sockets).
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in on off strict on-asid verify; do bash "$HERE/series2.sh" $m el0; done
echo SERIES-DONE
