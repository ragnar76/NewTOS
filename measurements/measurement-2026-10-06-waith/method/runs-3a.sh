#!/usr/bin/env bash
# runs-3a.sh <label> <cards-dir> -- the QEMU runs every stage-3a sub-step
# repeats. Product kernel (raspi4b, -append serial=on): desktop, TeraDesk,
# QED, Doom, APITEST (#Z, screendumps), S2ALL and PORT10 (serial verdicts).
# Scratch kernels: CRASHTST and the stage-3a probe UACC3A on raspi4b (crash
# kernel, IKBD injector answers the prompts), KBDVEC + EmuCON (keys kernel),
# virt crash kernel (CRCHILD, CRASHTST, UACC3A from the RAM registry; prompts
# answered over serial), virt selftests, virt product desktop.
set -uo pipefail
L="$1"; C="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
K="$B/kernel8-$L.img"; KV="$B/kernel-virt-$L.img"
mkdir -p "$C"
sd() { bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd s2all 'C:\S2ALL.NAP'
sd p10 'C:\P10TEST.NCP'; sd crash 'C:\CRASHTST.NAP'; sd kbdvec 'C:\KBDVEC.NAP'
sd uacc 'C:\UACC3A.NAP'
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
r4 "$K" "$C/plain.img"    "$L-rpi4-desktop"  25:t25.png
r4 "$K" "$C/teradesk.img" "$L-rpi4-teradesk" 35:t35.png
r4 "$K" "$C/qed.img"      "$L-rpi4-qed"      30:t30.png
r4 "$K" "$C/doom.img"     "$L-rpi4-doom"     30:t30.png 45:t45.png
r4 "$K" "$C/apitest.img"  "$L-rpi4-apitest"  25:t25.png 40:t40.png
rd "$K" "$C/s2all.img"    "$L-rpi4-s2all" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 240
rd "$K" "$C/p10.img"      "$L-rpi4-port10" --end 'port10: (ALL PASS|[0-9]+ of [0-9]+ checks FAILED)' --end-delay 2 --timeout 240
r4 "$B/kernel8-crash-$L.img" "$C/crash.img" "$L-rpi4-crashtst" 40:t40.png 70:t70.png
rd "$B/kernel8-crash-$L.img" "$C/uacc.img" "$L-rpi4-uacc3a" --end 'UACC3A: (PASS|FAIL)' --end-delay 8 --timeout 600
r4 "$B/kernel8-keys-$L.img" "$C/kbdvec.img" "$L-rpi4-kbdvec-emucon" 20:t20.png 35:t35.png 60:t60.png
bash "$HERE/run-virt.sh" "$B/kernel-virt-crash-$L.img" "$L-virt-crash" --answer --timeout 400
bash "$HERE/run-virt.sh" "$B/kernel-virt-st-$L.img" "$L-virt-selftests" --timeout 240
bash "$HERE/run-virt.sh" "$KV" "$L-virt-desktop" --timeout 120
