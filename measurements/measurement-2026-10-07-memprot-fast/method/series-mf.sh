#!/usr/bin/env bash
# series-mf.sh <cards-root> <scratch-root> [modes...] -- MEMPROT-FAST: the
# ARCH03-4 series (series-arch03-4.sh of measurement-2026-10-06-arch03-4,
# unchanged runs) with label mf and the verification mode on in every mode
# (mode.sh: memprot=verify appended). Default modes: on off strict on-asid.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
C="${1:?}"; R="${2:?}"; shift 2
B="$HERE/../bin"
L=mf
MODES="${*:-on off strict on-asid}"
for m in $MODES; do
  echo "=== $m $(date)"
  M="$m"; . "$HERE/mode.sh"
  bash "$HERE/pp-run.sh" $L "$C/$m" "$m"
  bash "$HERE/ppdesk-run.sh" $L "$R" "$m"
  bash "$HERE/runs-poste9.sh" $L "$C/$m" "$m"
  bash "$HERE/desk3a-run.sh" $L "$R" "$m"
  [ -f "$C/$m/vforkt.img" ] || bash "$HERE/seed-card.sh" "$C/$m/vforkt.img" 'C:\VFORKT.NAP' > /dev/null
  S2=s2all; [ "$BASE" = strict ] && S2=s2alls
  APPEND="$A4" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-crash-$L.img" "$C/$m/$S2.img" "$L-$m-rpi4-$S2-crashkernel" \
    --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 600
  APPEND="$A4" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-$L.img" "$C/$m/vforkt.img" "$L-$m-rpi4-vforkt-alone" \
    --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 400
  bash "$HERE/asid-runs.sh" $L "$C/$m" "$m"
done
echo "=== done $(date)"
