#!/usr/bin/env bash
# desk-acc.sh -- NEWDESK desktop with the SYSACC accessory (measurement-2026-10-13-libc-tty-acceptance/bin/SYSACC.NAC) and
# the m113 desktop INF (measurement-2026-10-18-proc04-curdir-test/method/m113/desk.inf); head vs wt, modes on/strict/off;
# screendumps at 25 and 40 s (to $S/shots, compared by sha256 in the caller). One QEMU at a time.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
M="$WS/measurements/measurement-2026-10-18-proc04-curdir-test/method/m113"
bash "$HERE/mkcard.sh" "$S/cards/dacc.img" "$M/desk.inf" ACC/SYSACC.NAC="$WS/measurements/measurement-2026-10-13-libc-tty-acceptance/bin/SYSACC.NAC" > "$S/cards/dacc.log" 2>&1
for side in head wt; do for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$S/bin/kernel8-$side-plain.img" "$S/cards/dacc.img" "rg-$side-$m-dacc" "$A" 'NEVER-ENDS' 42 25 40
done; done
