#!/usr/bin/env bash
# allseries.sh -- acceptance: series.sh for modes on off strict on-asid verify, EL0 then desk=kernel. cwd /tmp/m5j/a15
cd /tmp/m5j/a15
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in ${MODES:-on off strict on-asid verify}; do
  bash "$HERE/series.sh" acc $m
  DESK=kernel bash "$HERE/series.sh" acc $m
done
