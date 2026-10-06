#!/usr/bin/env bash
# pp-run.sh <label> <cards-dir> <on|off|strict> -- TASK/PPARENT probe runs on
# the product kernel (raspi4b, serial=on + the mode's memprot argument):
#   pparent: ONE boot covers four launch paths -- C:\AUTO\PPAUTO.NCP (AUTO,
#     parent = initial_basepage, env {0,0}), C:\ACC\PPACC.NAC (libc accessory,
#     p_parent 0, env = AES env) next to C:\ACC\SYSACC.NAC (plain-crt0
#     accessory), #Z C:\PPZ.NAP (parent = AES PD, env = AES env, shel_envrn
#     into the AES PD's p_env) which runs itself as a Pexec child
#     (p_parent = its PD). Ends at "PPARENT Z: PASS|FAIL".
#   envrn: #Z C:\ENVRN.NAP -- shel_envrn from EL0 with EmuTOS's default_env.
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
[ -f "$C/pparent.img" ] || SEED_ACC=1 EXTRA="AUTO/PPAUTO.NCP=$B/PPAUTO.NCP ACC/PPACC.NAC=$B/PPACC.NAC" \
  bash "$HERE/seed-card.sh" "$C/pparent.img" 'C:\PPZ.NAP' > /dev/null
[ -f "$C/envrn.img" ] || bash "$HERE/seed-card.sh" "$C/envrn.img" 'C:\ENVRN.NAP' > /dev/null
export APPEND="$A4"
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
rd "$K" "$C/pparent.img" "$T-rpi4-pparent" --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 300
rd "$K" "$C/envrn.img"   "$T-rpi4-envrn"   --end 'ENVRN: (PASS|FAIL)' --end-delay 2 --timeout 240
