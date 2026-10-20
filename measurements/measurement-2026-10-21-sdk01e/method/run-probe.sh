#!/usr/bin/env bash
# run-probe.sh <mode...> -- the RTP card on kernel8-plain, one QEMU per mode (on | strict | off), 90 s max, ends on
# "desktop up" (RTP RESULT comes before it, from C:\AUTO); serial -> ../serial/rt-<mode>.log; the clone is fsck'ed
# (../serial/rt-<mode>-fsck.txt) before it is deleted (run.sh FSCK=1). Pattern of measurement-2026-10-20-sdk01a.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
for m in "$@"; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  FSCK=1 bash "$HERE/run.sh" "$S/bin/kernel8-plain.img" "$S/cards/rt.img" "rt-$m" "$A" 'desktop up' 90
  L="$HERE/../serial/rt-$m.log"
  echo "  $(grep -a 'RTP RESULT' "$L" | tr -d '\r')  FAIL lines: $(grep -ac '\[RTP\] FAIL' "$L")  ok lines: $(grep -ac '\[RTP\] ok' "$L")  desktop: $(grep -ac 'desktop up' "$L")  fsck: $(grep -E 'rc=' "$HERE/../serial/rt-$m-fsck.txt" | tr '\n' ' ')"
done
