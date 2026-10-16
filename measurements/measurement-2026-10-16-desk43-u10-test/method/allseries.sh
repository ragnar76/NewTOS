#!/usr/bin/env bash
# allseries.sh -- acceptance: series.sh for modes on off strict on-asid verify, EL0 then desk=kernel. cwd /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad
cd /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in ${MODES:-on off strict on-asid verify}; do
  bash "$HERE/series.sh" acc $m
  DESK=kernel bash "$HERE/series.sh" acc $m
done
