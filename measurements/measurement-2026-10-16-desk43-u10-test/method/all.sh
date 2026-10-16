#!/usr/bin/env bash
# all.sh -- DESK43 U9-5: every QEMU run of this directory, one at a time (cwd /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad). Logs to ../serial,
# screendumps to ../screendumps. Order: series (EL0 + desk=kernel, on/off/strict/on-asid), save, seq, apps, crash,
# selftest, qemu-virt.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in on off strict on-asid; do bash "$HERE/series.sh" acc $m; DESK=kernel bash "$HERE/series.sh" acc $m; done
bash "$HERE/save.sh" on
bash "$HERE/seq.sh"
bash "$HERE/apps.sh"
bash "$HERE/crash.sh"
bash "$HERE/selftest.sh"
bash "$HERE/virt.sh"
echo ALLDONE
