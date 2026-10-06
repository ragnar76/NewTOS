#!/usr/bin/env bash
# port09-runs.sh <scratch root> [part...] -- the QEMU raspi4b runs of
# TASK/PORT09 (serial=on; memprot on = no argument, strict =
# memprot=strict). Cards: seed-card.sh (private images in the scratch root,
# never tools/make-sd-image.sh). Parts:
#   pre      pre-change kernel + pre-change probes (done first, before any
#            source change; BIN=<scratch>/pre/probes): P9EPAR->P9EKID (can a
#            child read p_env?), P9APAR->ARGVTEST (argc 12 before)
#   probes   P9EPAR->P9EKID, P9APAR->ARGVTEST, P9PAR suite; on + strict
#   s2       S2ALL, VFORKT alone (on)
#   regr     PPARENT + ENVRN, APITEST, QED, TeraDesk, PORT10 (on)
#   td       TeraDesk F1 -> QED and TeraDesk F1 -> ARGVTEST (PT_ARGV launch,
#            tdkeys injector kernel), on
#   strict   S2ALLS on the crash kernel, memprot=strict
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; K="$B/kernel8-port09.img"; C="$S/port09-cards"   # own directory: the scratchpad's cards/ holds older runs' cards
mkdir -p "$C"
sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
for part in "$@"; do case "$part" in
pre)
  mkdir -p "$S/pre"
  BIN="$S/pre/probes" bash "$HERE/seed-card.sh" "$S/pre/envkid.img" 'C:\P9EPAR.NAP' > /dev/null
  BIN="$S/pre/probes" bash "$HERE/seed-card.sh" "$S/pre/argvtest.img" 'C:\P9APAR.NAP' > /dev/null
  APPEND=serial=on rd "$B/kernel8-pre.img" "$S/pre/envkid.img" pre-on-rpi4-envkid --end 'EPAR\] 3' --end-delay 2 --timeout 120
  APPEND=serial=on rd "$B/kernel8-pre.img" "$S/pre/argvtest.img" pre-on-rpi4-argvtest --end 'EPAR\] 3' --end-delay 2 --timeout 120
  ;;
probes)
  sd p9e 'C:\P9EPAR.NAP'; sd p9a 'C:\P9APAR.NAP'; sd p9 'C:\P9PAR.NAP'
  for M in on strict; do
    A="serial=on"; [ $M = strict ] && A="serial=on memprot=strict"
    APPEND="$A" rd "$K" "$C/p9e.img" "port09-$M-rpi4-envkid" --end 'EPAR\] 3' --end-delay 2 --timeout 120
    APPEND="$A" rd "$K" "$C/p9a.img" "port09-$M-rpi4-argvtest" --end 'EPAR\] 3' --end-delay 2 --timeout 120
    APPEND="$A" rd "$K" "$C/p9.img"  "port09-$M-rpi4-p9par" --end 'P9PAR: (PASS|FAIL)' --end-delay 2 --timeout 240
  done
  ;;
s2)
  export APPEND=serial=on
  sd s2all 'C:\S2ALL.NAP'; sd vforkt 'C:\VFORKT.NAP'
  rd "$K" "$C/s2all.img"  port09-on-rpi4-s2all  --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  rd "$K" "$C/vforkt.img" port09-on-rpi4-vforkt --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
  ;;
regr)
  export APPEND=serial=on
  sd apitest 'C:\APITEST.NCP'
  sd qed 'C:\Apps\QED\QED.NAP'; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd p10 'C:\P10TEST.NCP'
  bash "$HERE/pp-run.sh" port09 "$C" on
  r4 "$K" "$C/apitest.img"  port09-on-rpi4-apitest  25:t25.png 40:t40.png
  r4 "$K" "$C/qed.img"      port09-on-rpi4-qed      30:t30.png
  r4 "$K" "$C/teradesk.img" port09-on-rpi4-teradesk 35:t35.png
  rd "$K" "$C/p10.img"      port09-on-rpi4-port10 --end 'port10: (ALL PASS|[0-9]+ of [0-9]+ checks FAILED)' --end-delay 2 --timeout 240
  ;;
td)
  export APPEND=serial=on
  [ -f "$C/tdqed.img" ] || TDINF="$HERE/TERADESK-QED.INF" bash "$HERE/seed-card.sh" "$C/tdqed.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
  [ -f "$C/tdargv.img" ] || TDINF="$HERE/TERADESK-ARGV.INF" bash "$HERE/seed-card.sh" "$C/tdargv.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
  r4 "$B/kernel8-tdkeys-port09.img" "$C/tdqed.img"  port09-on-rpi4-tdqed  30:t30.png 50:t50.png
  r4 "$B/kernel8-tdkeys-port09.img" "$C/tdargv.img" port09-on-rpi4-tdargv 30:t30.png 50:t50.png
  ;;
strict)
  sd s2alls 'C:\S2ALLS.NAP'
  APPEND="serial=on memprot=strict" rd "$B/kernel8-crash-port09.img" "$C/s2alls.img" port09-strict-rpi4-s2alls-crashkernel --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  ;;
*) echo "part? $part"; exit 2;;
esac; done
