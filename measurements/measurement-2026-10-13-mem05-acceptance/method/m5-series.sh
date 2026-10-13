#!/usr/bin/env bash
# m5-series.sh <label> <cards-root> <mode> -- MEM05 part A, QEMU raspi4b, one protection mode
# (mode.sh: on | off | strict | on-asid; always serial=on). cwd = /tmp/m5j (short socket dir s/).
# Cards are shared by all modes (content identical; every run copies its card). Recipes of the
# regression sample copied from measurement-2026-10-13-libc-tty-acceptance/method/cm-series.sh +
# cm-batch2.sh (same tags with prefix <label> instead of tta/cm, same screendump offsets):
#   MEM05 probes: MDPROBE (default limit), MDPAR+MDKID (mdlimit=64), MDBUD (mdlimit=0: the page
#   budget ends it; desktop afterwards), all with --stamp for wall-clock timing.
#   Sample: desktop, accessories (SYSACC), TeraDesk, QED, Doom, APITEST, TeraDesk->QED (tdkeys
#   kernel), KBDVEC + EmuCON (keys kernel), S2ALLM (strict: S2ALLSM on the crash kernel), E9MEM.
# ONLY=<list of run names> limits the runs (e.g. ONLY="apitest mdprobe").
set -uo pipefail
L="${1:?}"; C="${2:?}"; m="${3:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; BM="$B/m"; K="$B/kernel8-$L.img"
M="$m"; . "$HERE/mode.sh"
T="$L-$m"; mkdir -p "$C"
want() { [ -z "${ONLY:-}" ] || [[ " $ONLY " == *" $1 "* ]]; }
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd kbdvec 'C:\KBDVEC.NAP'; sd e9mem 'C:\E9MEM.NAP'
sd mdprobe 'C:\MDPROBE.NAP'; sd mdpar 'C:\MDPAR.NAP'; sd mdbud 'C:\MDBUD.NAP'
[ -f "$C/acc.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/acc.img" - > /dev/null
[ -f "$C/tdqed.img" ] || TDINF="$HERE/TERADESK-QED.INF" bash "$HERE/seed-card.sh" "$C/tdqed.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
S2=s2all; KS="$K"; [ "$BASE" = strict ] && { S2=s2alls; KS="$B/kernel8-crash-$L.img"; }
[ -f "$C/${S2}m.img" ] || EXTRA="ENVKID.NAP=$BM/ENVKID.NAP" bash "$HERE/seed-card.sh" "$C/${S2}m.img" "C:\\$(echo $S2 | tr a-z A-Z).NAP" > /dev/null
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
echo "=== $T $(date) APPEND=$A4"
export APPEND="$A4"
want mdprobe && rd "$K" "$C/mdprobe.img" "$T-rpi4-mdprobe" --end 'MDPROBE: DONE' --end-delay 3 --timeout 3000 --stamp
want mdpar && APPEND="$A4 mdlimit=${MDPAR_LIMIT:-64}" rd "$K" "$C/mdpar.img" "$T-rpi4-mdpar" --end 'MDPAR: (PASS|FAIL)' --end-delay 3 --timeout 600 --stamp
want mdbud && APPEND="$A4 mdlimit=0" rd "$K" "$C/mdbud.img" "$T-rpi4-mdbud" --end '\[desk\] desktop up' --end-delay 8 --timeout 5000 --stamp
want apitest && r4 "$K" "$C/apitest.img" "$T-rpi4-apitest" 25:t25.png 40:t40.png
want desktop && r4 "$K" "$C/plain.img" "$T-rpi4-desktop" 25:t25.png
want acc && r4 "$K" "$C/acc.img" "$T-rpi4-acc" 25:t25.png
want teradesk && r4 "$K" "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
want qed && r4 "$K" "$C/qed.img" "$T-rpi4-qed" 30:t30.png
want doom && r4 "$K" "$C/doom.img" "$T-rpi4-doom" 30:t30.png 45:t45.png
want tdqed && r4 "$B/kernel8-tdkeys-$L.img" "$C/tdqed.img" "$T-rpi4-tdqed" 30:t30.png 50:t50.png
want emucon && r4 "$B/kernel8-keys-$L.img" "$C/kbdvec.img" "$T-rpi4-kbdvec-emucon" 20:t20.png 35:t35.png 60:t60.png
want s2allm && rd "$KS" "$C/${S2}m.img" "$T-rpi4-${S2}m" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
want e9mem && rd "$K" "$C/e9mem.img" "$T-rpi4-e9mem" --end '\[E9MEM\] E9MEM: (PASS|FAIL)' --end-delay 2 --timeout 240
echo "=== $T done $(date)"
