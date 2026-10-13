#!/usr/bin/env bash
# series-proc02.sh <cards-root> <scratch-root> -- TASK/PROC02: series-pp.sh of
# measurement-2026-10-06-pparent with label proc02 (the PPARENT probe runs,
# the desktop double-click launch, the full post-E9 series incl. PORT10, desk3a,
# S2ALL/S2ALLS on the crash kernel, VFORKT alone), modes on/off/strict.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
C="${1:?}"; R="${2:?}"
B="$HERE/../bin"
L=proc02
for m in on off strict; do
  echo "=== $m $(date)"
  bash "$HERE/pp-run.sh" $L "$C/$m" "$m"
  bash "$HERE/ppdesk-run.sh" $L "$R" "$m"
  bash "$HERE/runs-poste9.sh" $L "$C/$m" "$m"
  bash "$HERE/desk3a-run.sh" $L "$R" "$m"
  case "$m" in on) A="serial=on";; off) A="serial=on memprot=off";; strict) A="serial=on memprot=strict";; esac
  [ -f "$C/$m/vforkt.img" ] || bash "$HERE/seed-card.sh" "$C/$m/vforkt.img" 'C:\VFORKT.NAP' > /dev/null
  S2=s2all; [ "$m" = strict ] && S2=s2alls
  APPEND="$A" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-crash-$L.img" "$C/$m/$S2.img" "$L-$m-rpi4-$S2-crashkernel" \
    --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  APPEND="$A" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-$L.img" "$C/$m/vforkt.img" "$L-$m-rpi4-vforkt-alone" \
    --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
done
echo "=== done $(date)"
