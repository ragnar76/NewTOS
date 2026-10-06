#!/usr/bin/env bash
# runs-e9.sh <label> <cards-dir> <on|off|strict> -- the QEMU series of a
# E9 checkpoint in one protection mode (boot argument: on = none,
# off = memprot=off, strict = memprot=strict; raspi4b always serial=on).
# Stage 3c runs the FULL series in every mode (3b ran a reduced strict set):
#   product kernel (raspi4b): desktop, TeraDesk, QED, Doom, APITEST (#Z,
#     screendumps), S2ALL and PORT10 (serial verdicts), the desk accessory;
#   crash kernel: CRASHTST, UACC3A, MPROT (variant for the mode), E10 (new,
#     variant for the mode, end screendump shows the CROSSED userdef box);
#   keys kernel: KBDVEC + EmuCON;
#   virt: crash kernel (CRCHILD, CRASHTST, UACC3A, MPROT, then E10 from the
#     RAM registry), selftests, product desktop.
set -uo pipefail
L="$1"; C="$2"; M="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
K="$B/kernel8-$L.img"; KV="$B/kernel-virt-$L.img"
. "$HERE/mode.sh"
case "$BASE" in on) MP='C:\MPROT3B.NAP'; EP='C:\E10USR.NAP';; off) MP='C:\MPROTO.NAP'; EP='C:\E10USRO.NAP';;
  strict) MP='C:\MPROTS.NAP'; EP='C:\E10USRS.NAP';; esac
T="$L-$M"
mkdir -p "$C"
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd s2all 'C:\S2ALL.NAP'
sd p10 'C:\P10TEST.NCP'; sd crash 'C:\CRASHTST.NAP'; sd kbdvec 'C:\KBDVEC.NAP'
sd uacc 'C:\UACC3A.NAP'; sd "mprot-$M" "$MP"; sd "e10-$M" "$EP"
[ -f "$C/acc.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/acc.img" - > /dev/null
sd e9mem 'C:\E9MEM.NAP'; sd e9plain 'C:\E9PLAIN.NCP'
export APPEND="$A4"
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
rv() { APPEND="$AV" bash "$HERE/run-virt.sh" "$@"; }
r4 "$K" "$C/plain.img"    "$T-rpi4-desktop"  25:t25.png
r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
r4 "$K" "$C/qed.img"      "$T-rpi4-qed"      30:t30.png
r4 "$K" "$C/doom.img"     "$T-rpi4-doom"     30:t30.png 45:t45.png
r4 "$K" "$C/apitest.img"  "$T-rpi4-apitest"  25:t25.png 40:t40.png
r4 "$K" "$C/acc.img"      "$T-rpi4-acc"      25:t25.png
rd "$K" "$C/e9mem.img"    "$T-rpi4-e9mem" --end '\[E9MEM\] E9MEM: (PASS|FAIL)' --end-delay 2 --timeout 240
rd "$K" "$C/e9plain.img"  "$T-rpi4-e9plain" --end 'E9PLAIN: (PASS|FAIL)' --end-delay 2 --timeout 240
rd "$K" "$C/s2all.img"    "$T-rpi4-s2all" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 240
rd "$K" "$C/p10.img"      "$T-rpi4-port10" --end 'port10: (ALL PASS|[0-9]+ of [0-9]+ checks FAILED)' --end-delay 2 --timeout 240
r4 "$B/kernel8-crash-$L.img" "$C/crash.img" "$T-rpi4-crashtst" 40:t40.png 70:t70.png
rd "$B/kernel8-crash-$L.img" "$C/uacc.img" "$T-rpi4-uacc3a" --end 'UACC3A: (PASS|FAIL)' --end-delay 8 --timeout 600
rd "$B/kernel8-crash-$L.img" "$C/mprot-$M.img" "$T-rpi4-mprot" --end 'MPROT3B: (PASS|FAIL)' --end-delay 8 --timeout 300
rd "$B/kernel8-crash-$L.img" "$C/e10-$M.img" "$T-rpi4-e10" --end 'E10USR: (PASS|FAIL)' --end-delay 3 --timeout 300
r4 "$B/kernel8-keys-$L.img" "$C/kbdvec.img" "$T-rpi4-kbdvec-emucon" 20:t20.png 35:t35.png 60:t60.png
rv "$B/kernel-virt-crash-$L.img" "$T-virt-crash" --answer --end 'E10USR: (PASS|FAIL)' --end-delay 1.5 --timeout 700
rv "$B/kernel-virt-st-$L.img" "$T-virt-selftests" --timeout 240
rv "$KV" "$T-virt-desktop" --timeout 120
