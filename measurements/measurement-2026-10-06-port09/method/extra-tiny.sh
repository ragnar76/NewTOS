#!/usr/bin/env bash
# extra-tiny.sh <label> <cards-root> -- ARCH03-4: the forced-rollover hook
# (asid=tiny, 3 ASIDs per generation) in the two other protection modes, on
# top of the full on-tiny series: the ASID probes (asid-runs.sh), S2ALL
# (S2ALLS for strict) on the crash kernel, PPARENT, VFORKT alone.
set -uo pipefail
L="$1"; CR="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
for M in off-tiny strict-tiny; do
  echo "=== $M $(date)"
  . "$HERE/mode.sh"
  C="$CR/$M"; mkdir -p "$C"
  bash "$HERE/asid-runs.sh" "$L" "$C" "$M"
  S2=s2all; S2P='C:\S2ALL.NAP'; [ "$BASE" = strict ] && { S2=s2alls; S2P='C:\S2ALLS.NAP'; }
  [ -f "$C/$S2.img" ] || bash "$HERE/seed-card.sh" "$C/$S2.img" "$S2P" > /dev/null
  [ -f "$C/vforkt.img" ] || bash "$HERE/seed-card.sh" "$C/vforkt.img" 'C:\VFORKT.NAP' > /dev/null
  bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-crash-$L.img" "$C/$S2.img" "$L-$M-rpi4-$S2-crashkernel" \
    --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-$L.img" "$C/vforkt.img" "$L-$M-rpi4-vforkt-alone" \
    --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
  bash "$HERE/pp-run.sh" "$L" "$C" "$M"
done
echo "=== done $(date)"
