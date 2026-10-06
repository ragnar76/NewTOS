#!/usr/bin/env bash
# series-arch03-4.sh <cards-root> <scratch-root> [modes...] -- TASK ARCH03-4:
# the full PROC02 series (series-proc02.sh: PPARENT probes, desktop
# double-click, runs-poste9 = desktop, TeraDesk, QED, Doom, APITEST,
# accessory, E9MEM/E9PLAIN, S2ALL, PORT10, CRASHTST, UACC3A, MPROT, E10,
# KBDVEC+EmuCON, TeraDesk->QED, EmuCON five programs, ATEXIT, ACCFREE,
# MDPROBE, strict S2ALLS, virt crash/selftests/desktop; desk3a; S2ALL(S) on
# the crash kernel; VFORKT alone) plus the ASID probe runs (asid-runs.sh),
# per mode. Default modes: on off strict on-asid off-asid strict-asid on-tiny.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
C="${1:?}"; R="${2:?}"; shift 2
B="$HERE/../bin"
L=a4
MODES="${*:-on off strict on-asid off-asid strict-asid on-tiny}"
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
    --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  APPEND="$A4" bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-$L.img" "$C/$m/vforkt.img" "$L-$m-rpi4-vforkt-alone" \
    --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
  bash "$HERE/asid-runs.sh" $L "$C/$m" "$m"
done
echo "=== done $(date)"
