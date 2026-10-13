#!/usr/bin/env bash
# acc-series.sh <label> <cards-root> <mode> <group> -- MEM05 acceptance, QEMU raspi4b, one protection mode
# (mode.sh: on | off | strict | verify | on-asid; always serial=on). cwd = /tmp/m5j/acc (short socket dir s/).
# Groups:  seed   : seed every shared card (run once, serially, before the chains)
#          timed  : hellomem hellold mdacc mdstep mdtime mdprobe (run one mode at a time: wall-clock timings)
#          main   : MDPAR (mdlimit=64) MDBUD (mdlimit=0) APITEST + regression sample + extras
#          crash  : crash-kernel probes CRASHTST UACC3A MPROT E10USR (raspi4b)
#          verify : verify-mode subset (MDPAR mdlimit=400, APITEST, desktop, S2ALLM, E9MEM)
#          asid   : on-asid subset (APITEST, MDSTEP, MDPAR, desktop)
# ONLY=<names> limits runs inside a group.
set -uo pipefail
L="${1:?}"; C="${2:?}"; m="${3:?}"; G="${4:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; BM="$B/m"; BJ="$B/j"; K="$B/kernel8-$L.img"
M="$m"; . "$HERE/mode.sh"
T="$L-$m"; mkdir -p "$C"
want() { [ -z "${ONLY:-}" ] || [[ " $ONLY " == *" $1 "* ]]; }
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
case "$BASE" in on|verify) MP='C:\MPROT3B.NAP'; EP='C:\E10USR.NAP'; MPN=on;; off) MP='C:\MPROTO.NAP'; EP='C:\E10USRO.NAP'; MPN=off;;
  strict) MP='C:\MPROTS.NAP'; EP='C:\E10USRS.NAP'; MPN=strict;; esac
S2=s2all; KS="$K"; [ "$BASE" = strict ] && { S2=s2alls; KS="$B/kernel8-crash-$L.img"; }
U2="$(echo $S2 | tr a-z A-Z)"
if [ "$G" = seed ]; then
  sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
  sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'; sd kbdvec 'C:\KBDVEC.NAP'; sd e9mem 'C:\E9MEM.NAP'
  sd mdprobe 'C:\MDPROBE.NAP'; sd mdstep 'C:\MDSTEP.NAP'; sd mdtime 'C:\MDTIME.NAP'; sd mdpar 'C:\MDPAR.NAP'; sd mdbud 'C:\MDBUD.NAP'
  sd hellomem 'C:\HELLOMEM.NAP'; sd hellold 'C:\HELLOOLD.NAP'
  [ -f "$C/mdacc.img" ] || EXTRA="ACC/MDACC.NAC=$B/MDACC.NAC" bash "$HERE/seed-card.sh" "$C/mdacc.img" - > /dev/null
  [ -f "$C/acc.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/acc.img" - > /dev/null
  [ -f "$C/accfree.img" ] || SEED_ACC=1 bash "$HERE/seed-card.sh" "$C/accfree.img" 'C:\ACCFREE.NAP' > /dev/null
  [ -f "$C/tdqed.img" ] || TDINF="$HERE/TERADESK-QED.INF" bash "$HERE/seed-card.sh" "$C/tdqed.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
  [ -f "$C/pparent.img" ] || SEED_ACC=1 EXTRA="AUTO/PPAUTO.NCP=$B/PPAUTO.NCP ACC/PPACC.NAC=$B/PPACC.NAC" bash "$HERE/seed-card.sh" "$C/pparent.img" 'C:\PPZ.NAP' > /dev/null
  for s in s2all s2alls; do U="$(echo $s | tr a-z A-Z)"; [ -f "$C/${s}m.img" ] || EXTRA="ENVKID.NAP=$BM/ENVKID.NAP" bash "$HERE/seed-card.sh" "$C/${s}m.img" "C:\\$U.NAP" > /dev/null; done
  sd stdoutdb 'C:\STDOUTDB.NAP'; sd serr 'C:\STDERRP.NAP'; sd stderra 'C:\STDERRA.NAP'
  [ -f "$C/serr-oldcrt0.img" ] || EXTRA="STDERRP.NAP=$B/oldcrt0/STDERRP.NAP" bash "$HERE/seed-card.sh" "$C/serr-oldcrt0.img" 'C:\STDERRP.NAP' > /dev/null
  sd ttya 'C:\TTYA.NAP'; sd ttyprobe 'C:\TTYPROBE.NAP'
  sd crash 'C:\CRASHTST.NAP'; sd uacc 'C:\UACC3A.NAP'; sd pflags1 'C:\PFLAGS1.NCP'; sd pflags2 'C:\PFLAGS2.NCP'; sd tsrpar 'C:\TSRPAR.NAP'
  sd mprot-on 'C:\MPROT3B.NAP'; sd mprot-off 'C:\MPROTO.NAP'; sd mprot-strict 'C:\MPROTS.NAP'
  sd e10-on 'C:\E10USR.NAP'; sd e10-off 'C:\E10USRO.NAP'; sd e10-strict 'C:\E10USRS.NAP'
  echo seeded; exit 0
fi
echo "=== $T $G $(date) APPEND=$A4"
export APPEND="$A4"
case "$G" in
timed)
  want hellomem && rd "$K" "$C/hellomem.img" "$T-rpi4-hellomem" --end 'HELLOMEM: DONE' --end-delay 2 --timeout 300 --stamp
  want hellold && rd "$K" "$C/hellold.img" "$T-rpi4-hellold" --end 'HELLOMEM: DONE' --end-delay 2 --timeout 300 --stamp
  want mdacc && APPEND="$A4 mdlimit=128" rd "$K" "$C/mdacc.img" "$T-rpi4-mdacc" --end '\[desk\] desktop up' --end-delay 8 --timeout 900 --stamp
  want mdstep && APPEND="$A4 mdlimit=128" rd "$K" "$C/mdstep.img" "$T-rpi4-mdstep" --end 'MDSTEP: DONE' --end-delay 3 --timeout 3000 --stamp
  want mdtime && rd "$K" "$C/mdtime.img" "$T-rpi4-mdtime" --end 'MDSTEP: DONE' --end-delay 3 --timeout 3000 --stamp
  want mdprobe && rd "$K" "$C/mdprobe.img" "$T-rpi4-mdprobe" --end 'MDPROBE: DONE' --end-delay 3 --timeout 3000 --stamp
  ;;
main)
  want mdpar && APPEND="$A4 mdlimit=64" rd "$K" "$C/mdpar.img" "$T-rpi4-mdpar" --end 'MDPAR: (PASS|FAIL)' --end-delay 3 --timeout 600 --stamp
  want mdbud && APPEND="$A4 mdlimit=0" rd "$K" "$C/mdbud.img" "$T-rpi4-mdbud" --end '\[desk\] desktop up' --end-delay 8 --timeout 5000 --stamp
  want mdbud2 && APPEND="$A4 mdlimit=0" rd "$K" "$C/mdbud.img" "$T-rpi4-mdbud-rerun" --end '\[desk\] desktop up' --end-delay 8 --timeout 5000 --stamp
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
  want accfree && rd "$K" "$C/accfree.img" "$T-rpi4-accfree" --end 'ACCFREE: (PASS|FAIL)' --end-delay 2 --timeout 400
  want pflags1 && rd "$K" "$C/pflags1.img" "$T-rpi4-pflags1" --end 'PFLAGS: (PASS|FAIL)' --end-delay 2 --timeout 200
  want pflags2 && rd "$K" "$C/pflags2.img" "$T-rpi4-pflags2" --end 'PFLAGS: (PASS|FAIL)' --end-delay 2 --timeout 200
  want tsr && rd "$KS" "$C/tsrpar.img" "$T-rpi4-tsrpar" --end 'TSR: (PASS|FAIL)' --end-delay 3 --timeout 300
  want pparent && rd "$K" "$C/pparent.img" "$T-rpi4-pparent" --end 'PPARENT Z: (PASS|FAIL)' --end-delay 3 --timeout 300
  want stdoutdb && rd "$K" "$C/stdoutdb.img" "$T-rpi4-stdoutdb" --end 'DBG: DONE' --end-delay 2 --timeout 200
  want stderr && rd "$K" "$C/serr.img" "$T-rpi4-stderr-newcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
  want stderr && rd "$K" "$C/serr-oldcrt0.img" "$T-rpi4-stderr-oldcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
  want stderr && rd "$K" "$C/stderra.img" "$T-rpi4-stderra" --end 'STDERRA\[PAR\]: DONE' --end-delay 3 --timeout 150
  want ttya && rd "$K" "$C/ttya.img" "$T-rpi4-ttya" --end 'TTYPROBE: (PASS|FAIL)' --end-delay 2 --timeout 200
  want port10 && bash "$HERE/rerun-port10.sh" "$L" "$C/p10" "$m"
  ;;
crash)
  KC="$B/kernel8-crash-$L.img"
  want crashtst && r4 "$KC" "$C/crash.img" "$T-rpi4-crashtst" 40:t40.png 70:t70.png
  want uacc && rd "$KC" "$C/uacc.img" "$T-rpi4-uacc3a" --end 'UACC3A: (PASS|FAIL)' --end-delay 8 --timeout 600
  want mprot && rd "$KC" "$C/mprot-$MPN.img" "$T-rpi4-mprot" --end 'MPROT3B: (PASS|FAIL)' --end-delay 8 --timeout 300
  want e10 && rd "$KC" "$C/e10-$MPN.img" "$T-rpi4-e10" --end 'E10USR: (PASS|FAIL)' --end-delay 3 --timeout 300
  ;;
verify)
  want mdpar && APPEND="$A4 mdlimit=400" rd "$K" "$C/mdpar.img" "$T-rpi4-mdpar" --end 'MDPAR: (PASS|FAIL)' --end-delay 3 --timeout 3000 --stamp
  want apitest && r4 "$K" "$C/apitest.img" "$T-rpi4-apitest" 25:t25.png 40:t40.png
  want desktop && r4 "$K" "$C/plain.img" "$T-rpi4-desktop" 25:t25.png
  want s2allm && rd "$K" "$C/${S2}m.img" "$T-rpi4-${S2}m" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 600
  want e9mem && rd "$K" "$C/e9mem.img" "$T-rpi4-e9mem" --end '\[E9MEM\] E9MEM: (PASS|FAIL)' --end-delay 2 --timeout 400
  ;;
asid)
  APPEND="$A4 mdlimit=128" rd "$K" "$C/mdstep.img" "$T-rpi4-mdstep" --end 'MDSTEP: DONE' --end-delay 3 --timeout 3000 --stamp
  APPEND="$A4 mdlimit=64" rd "$K" "$C/mdpar.img" "$T-rpi4-mdpar" --end 'MDPAR: (PASS|FAIL)' --end-delay 3 --timeout 600 --stamp
  r4 "$K" "$C/apitest.img" "$T-rpi4-apitest" 25:t25.png 40:t40.png
  r4 "$K" "$C/plain.img" "$T-rpi4-desktop" 25:t25.png
  ;;
esac
echo "=== $T $G done $(date)"
