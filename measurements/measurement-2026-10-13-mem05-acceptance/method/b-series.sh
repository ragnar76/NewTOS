#!/usr/bin/env bash
# b-series.sh <label> <cards-root> <mode> -- MEM05 part B (SDK MAXHUNK 16 MiB), QEMU raspi4b, one
# protection mode (mode.sh: on | off | strict | on-asid; always serial=on). cwd = /tmp/m5j (short
# socket dir s/). Copied from m5-series.sh (part A); kernels = byte copies of part A's kernels
# (bin/KERNELS-REUSED.SHA256: the kernel is unchanged by part B), every program relinked against
# the part-B libc.a (build-all.sh). Cards shared by all modes (every run copies its card).
#   Part B probes: MDPROBE (part A's source, default limit, 3 000 000 x malloc(32), timed),
#   MDSTEP (mdlimit=128: MDs + mintlib hunks per step up to 10 000 000), MDTIME (same series
#   without probes, default limit, timed), HELLOMEM/HELLOOLD (footprint, new/old libc),
#   MDACC (accessory C:\ACC\MDACC.NAC, mdlimit=128; design QEMU test 15).
#   Sample: as part A (desktop, acc, TeraDesk, QED, Doom, APITEST, TeraDesk->QED, KBDVEC+EmuCON,
#   S2ALLM/S2ALLSM, E9MEM).
# ONLY=<list of run names> limits the runs.
set -uo pipefail
L="${1:?}"; C="${2:?}"; m="${3:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; BM="$B/m"; K="$B/kernel8-m5.img"
M="$m"; . "$HERE/mode.sh"
T="$L-$m"; mkdir -p "$C"
want() { [ -z "${ONLY:-}" ] || [[ " $ONLY " == *" $1 "* ]]; }
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd kbdvec 'C:\KBDVEC.NAP'; sd e9mem 'C:\E9MEM.NAP'
sd mdprobe 'C:\MDPROBE.NAP'; sd mdstep 'C:\MDSTEP.NAP'; sd mdtime 'C:\MDTIME.NAP'
sd hellomem 'C:\HELLOMEM.NAP'; sd hellold 'C:\HELLOOLD.NAP'
[ -f "$C/mdacc.img" ] || EXTRA="ACC/MDACC.NAC=$B/MDACC.NAC" bash "$HERE/seed-card.sh" "$C/mdacc.img" - > /dev/null
[ -f "$C/acc.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/acc.img" - > /dev/null
[ -f "$C/tdqed.img" ] || TDINF="$HERE/TERADESK-QED.INF" bash "$HERE/seed-card.sh" "$C/tdqed.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
S2=s2all; KS="$K"; [ "$BASE" = strict ] && { S2=s2alls; KS="$B/kernel8-crash-m5.img"; }
[ -f "$C/${S2}m.img" ] || EXTRA="ENVKID.NAP=$BM/ENVKID.NAP" bash "$HERE/seed-card.sh" "$C/${S2}m.img" "C:\\$(echo $S2 | tr a-z A-Z).NAP" > /dev/null
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
echo "=== $T $(date) APPEND=$A4"
export APPEND="$A4"
want hellomem && rd "$K" "$C/hellomem.img" "$T-rpi4-hellomem" --end 'HELLOMEM: DONE' --end-delay 2 --timeout 300 --stamp
want hellold && rd "$K" "$C/hellold.img" "$T-rpi4-hellold" --end 'HELLOMEM: DONE' --end-delay 2 --timeout 300 --stamp
want mdacc && APPEND="$A4 mdlimit=128" rd "$K" "$C/mdacc.img" "$T-rpi4-mdacc" --end '\[desk\] desktop up' --end-delay 8 --timeout 900 --stamp
want mdstep && APPEND="$A4 mdlimit=128" rd "$K" "$C/mdstep.img" "$T-rpi4-mdstep" --end 'MDSTEP: DONE' --end-delay 3 --timeout 3000 --stamp
want mdtime && rd "$K" "$C/mdtime.img" "$T-rpi4-mdtime" --end 'MDSTEP: DONE' --end-delay 3 --timeout 3000 --stamp
want mdprobe && rd "$K" "$C/mdprobe.img" "$T-rpi4-mdprobe" --end 'MDPROBE: DONE' --end-delay 3 --timeout 3000 --stamp
want apitest && r4 "$K" "$C/apitest.img" "$T-rpi4-apitest" 25:t25.png 40:t40.png
want desktop && r4 "$K" "$C/plain.img" "$T-rpi4-desktop" 25:t25.png
want acc && r4 "$K" "$C/acc.img" "$T-rpi4-acc" 25:t25.png
want teradesk && r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
want qed && r4 "$K" "$C/qed.img" "$T-rpi4-qed" 30:t30.png
want doom && r4 "$K" "$C/doom.img" "$T-rpi4-doom" 30:t30.png 45:t45.png
want tdqed && r4 "$B/kernel8-tdkeys-m5.img" "$C/tdqed.img" "$T-rpi4-tdqed" 30:t30.png 50:t50.png
want emucon && r4 "$B/kernel8-keys-m5.img" "$C/kbdvec.img" "$T-rpi4-kbdvec-emucon" 20:t20.png 35:t35.png 60:t60.png
want s2allm && rd "$KS" "$C/${S2}m.img" "$T-rpi4-${S2}m" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
want e9mem && rd "$K" "$C/e9mem.img" "$T-rpi4-e9mem" --end '\[E9MEM\] E9MEM: (PASS|FAIL)' --end-delay 2 --timeout 240
echo "=== $T done $(date)"
