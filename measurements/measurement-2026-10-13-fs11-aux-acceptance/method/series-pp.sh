#!/usr/bin/env bash
# series-pp.sh <cards-root> <scratch-root> -- TASK/PPARENT: the full post-E9
# QEMU series of measurement-2026-10-06-vforkt-fix (series-vf.sh: runs-poste9.sh,
# desk3a, S2ALL on the crash kernel, VFORKT alone) with label pp, plus per mode
# the PPARENT probe runs (pp-run.sh: AUTO, accessory, #Z, Pexec child, ENVRN)
# and the desktop double-click launch (ppdesk-run.sh).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
C="${1:?}"; R="${2:?}"
B="$HERE/../bin"
for m in on off strict; do
  echo "=== $m $(date)"
  bash "$HERE/pp-run.sh" pp "$C/$m" "$m"
  bash "$HERE/ppdesk-run.sh" pp "$R" "$m"
  bash "$HERE/runs-poste9.sh" pp "$C/$m" "$m"
  bash "$HERE/desk3a-run.sh" pp "$R" "$m"
  case "$m" in on) A="serial=on";; off) A="serial=on memprot=off";; strict) A="serial=on memprot=strict";; esac
  [ -f "$C/$m/vforkt.img" ] || bash "$HERE/seed-card.sh" "$C/$m/vforkt.img" 'C:\VFORKT.NAP' > /dev/null
  S2=s2all; [ "$m" = strict ] && S2=s2alls
  APPEND="$A" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-crash-pp.img" "$C/$m/$S2.img" "pp-$m-rpi4-$S2-crashkernel" \
    --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  APPEND="$A" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-pp.img" "$C/$m/vforkt.img" "pp-$m-rpi4-vforkt-alone" \
    --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
done
echo "=== done $(date)"
