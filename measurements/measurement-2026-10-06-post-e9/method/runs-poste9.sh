#!/usr/bin/env bash
# runs-poste9.sh <label> <cards-dir> <on|off|strict> -- the post-E9 QEMU
# series in one protection mode: the E9 series (runs-e9.sh: desktop,
# TeraDesk, QED, Doom, APITEST, accessory, E9MEM/E9PLAIN, S2ALL, PORT10,
# CRASHTST, UACC3A, MPROT, E10, KBDVEC+EmuCON, virt crash/selftests/desktop)
# and the E9 extra runs (TeraDesk -> QED, EmuCON five programs), plus:
#   ATEXITP  (point 2: exit() after main -- atexit + stdio flush)
#   ACCFREE  (point 3: free_accs gives the accessory memory back; #Z on a
#             card with C:\ACC\SYSACC.NAC, two AES sessions)
#   MDPROBE  (GEMDOS descriptors under malloc-via-Malloc)
#   strict: S2ALLS (the strict-expectation build) on the crash kernel,
#           whose injector answers the crash prompt of the denied children.
set -uo pipefail
L="$1"; C="$2"; M="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
K="$B/kernel8-$L.img"
case "$M" in
  on) A4="serial=on";; off) A4="serial=on memprot=off";; strict) A4="serial=on memprot=strict";;
  *) echo "mode?"; exit 2;;
esac
T="$L-$M"
mkdir -p "$C"
bash "$HERE/runs-e9.sh" "$L" "$C" "$M"
bash "$HERE/runs-e9-extra.sh" "$L" "$C" "$M"
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
sd atexit 'C:\ATEXITP.NAP'; sd mdprobe 'C:\MDPROBE.NAP'; sd s2alls 'C:\S2ALLS.NAP'
[ -f "$C/accfree.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/accfree.img" 'C:\ACCFREE.NAP' > /dev/null
export APPEND="$A4"
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
rd "$K" "$C/atexit.img"  "$T-rpi4-atexit"  --end 'ATEXIT: (PASS|FAIL)' --end-delay 2 --timeout 240
rd "$K" "$C/accfree.img" "$T-rpi4-accfree" --end 'ACCFREE: (PASS|FAIL)' --end-delay 2 --timeout 400
rd "$K" "$C/mdprobe.img" "$T-rpi4-mdprobe" --end 'MDPROBE: DONE' --end-delay 2 --timeout 600
if [ "$M" = strict ]; then
  rd "$B/kernel8-crash-$L.img" "$C/s2alls.img" "$T-rpi4-s2alls-crashkernel" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
fi
