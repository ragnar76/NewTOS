#!/usr/bin/env bash
# series-vf.sh <cards-root> <scratch-root> -- vforkt-fix: the full post-E9
# QEMU series (runs-poste9.sh = runs-e9.sh + runs-e9-extra.sh + ATEXIT,
# ACCFREE, MDPROBE, strict S2ALLS on the crash kernel) and desk3a, in all
# three protection modes, label vf (= series-p1.sh with the label changed),
# plus per mode: S2ALL on the crash kernel and VFORKT alone (#Z).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
C="${1:?}"; R="${2:?}"
B="$HERE/../bin"
for m in on off strict; do
  echo "=== $m $(date)"
  bash "$HERE/runs-poste9.sh" vf "$C/$m" "$m"
  bash "$HERE/desk3a-run.sh" vf "$R" "$m"
  case "$m" in on) A="serial=on";; off) A="serial=on memprot=off";; strict) A="serial=on memprot=strict";; esac
  [ -f "$C/$m/vforkt.img" ] || bash "$HERE/seed-card.sh" "$C/$m/vforkt.img" 'C:\VFORKT.NAP' > /dev/null
  S2=s2all; [ "$m" = strict ] && S2=s2alls
  APPEND="$A" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-crash-vf.img" "$C/$m/$S2.img" "vf-$m-rpi4-$S2-crashkernel" \
    --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  APPEND="$A" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-vf.img" "$C/$m/vforkt.img" "vf-$m-rpi4-vforkt-alone" \
    --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
done
echo "=== done $(date)"
