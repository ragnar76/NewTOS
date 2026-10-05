#!/usr/bin/env bash
# runs-3b.sh <label> <cards-dir> <on|off|strict> -- the QEMU series of a
# stage-3b checkpoint in one protection mode (boot argument: on = none,
# off = memprot=off, strict = memprot=strict; raspi4b always serial=on).
#   on/off: product kernel (raspi4b): desktop, TeraDesk, QED, Doom, APITEST
#     (#Z, screendumps), S2ALL and PORT10 (serial verdicts), the desk
#     accessory (C:\ACC\SYSACC.NAC on a plain desktop); crash kernel:
#     CRASHTST, UACC3A, MPROT (the variant for the mode); keys kernel: KBDVEC
#     + EmuCON; virt: crash kernel (CRCHILD, CRASHTST, UACC3A, MPROT from the
#     RAM registry), selftests, product desktop.
#   strict: the MPROT probe on raspi4b and virt, desktop + TeraDesk.
set -uo pipefail
L="$1"; C="$2"; M="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"
K="$B/kernel8-$L.img"; KV="$B/kernel-virt-$L.img"
case "$M" in
  on) A4="serial=on"; AV="";;
  off) A4="serial=on memprot=off"; AV="memprot=off";;
  strict) A4="serial=on memprot=strict"; AV="memprot=strict";;
  *) echo "mode?"; exit 2;;
esac
case "$M" in on) MP='C:\MPROT3B.NAP';; off) MP='C:\MPROTO.NAP';; strict) MP='C:\MPROTS.NAP';; esac
T="$L-$M"
mkdir -p "$C"
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd s2all 'C:\S2ALL.NAP'
sd p10 'C:\P10TEST.NCP'; sd crash 'C:\CRASHTST.NAP'; sd kbdvec 'C:\KBDVEC.NAP'
sd uacc 'C:\UACC3A.NAP'; sd "mprot-$M" "$MP"
[ -f "$C/acc.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/acc.img" - > /dev/null
export APPEND="$A4"
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
rv() { APPEND="$AV" bash "$HERE/run-virt.sh" "$@"; }
if [ "$M" = strict ]; then
  r4 "$K" "$C/plain.img"    "$T-rpi4-desktop"  25:t25.png
  r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
  rd "$B/kernel8-crash-$L.img" "$C/mprot-$M.img" "$T-rpi4-mprot" --end 'MPROT3B: (PASS|FAIL)' --end-delay 8 --timeout 300
  rv "$B/kernel-virt-crash-$L.img" "$T-virt-crash" --answer --end 'MPROT3B: (PASS|FAIL)' --end-delay 4 --timeout 500
  exit 0
fi
r4 "$K" "$C/plain.img"    "$T-rpi4-desktop"  25:t25.png
r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
r4 "$K" "$C/qed.img"      "$T-rpi4-qed"      30:t30.png
r4 "$K" "$C/doom.img"     "$T-rpi4-doom"     30:t30.png 45:t45.png
r4 "$K" "$C/apitest.img"  "$T-rpi4-apitest"  25:t25.png 40:t40.png
r4 "$K" "$C/acc.img"      "$T-rpi4-acc"      25:t25.png
rd "$K" "$C/s2all.img"    "$T-rpi4-s2all" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 240
rd "$K" "$C/p10.img"      "$T-rpi4-port10" --end 'port10: (ALL PASS|[0-9]+ of [0-9]+ checks FAILED)' --end-delay 2 --timeout 240
r4 "$B/kernel8-crash-$L.img" "$C/crash.img" "$T-rpi4-crashtst" 40:t40.png 70:t70.png
rd "$B/kernel8-crash-$L.img" "$C/uacc.img" "$T-rpi4-uacc3a" --end 'UACC3A: (PASS|FAIL)' --end-delay 8 --timeout 600
rd "$B/kernel8-crash-$L.img" "$C/mprot-$M.img" "$T-rpi4-mprot" --end 'MPROT3B: (PASS|FAIL)' --end-delay 8 --timeout 300
r4 "$B/kernel8-keys-$L.img" "$C/kbdvec.img" "$T-rpi4-kbdvec-emucon" 20:t20.png 35:t35.png 60:t60.png
rv "$B/kernel-virt-crash-$L.img" "$T-virt-crash" --answer --end 'MPROT3B: (PASS|FAIL)' --end-delay 4 --timeout 600
rv "$B/kernel-virt-st-$L.img" "$T-virt-selftests" --timeout 240
rv "$KV" "$T-virt-desktop" --timeout 120
