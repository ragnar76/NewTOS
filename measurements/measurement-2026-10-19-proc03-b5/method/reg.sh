#!/usr/bin/env bash
# reg.sh -- the regression QEMU runs of this step, in sequence (one QEMU at a time):
#   regress/series.sh wt                      APITEST, S2ALL/S2ALLS, VFORKT, TeraDesk->F1->QED (on/strict/off)
#   TeraDesk->F1->QED on the head tdq kernel  pixel reference for the wt screendumps
#   PROC03 stage-1 probe (s1/, inj kernel)    on/strict/off
#   A113 idprobe (a113/)                       on
#   desk-acc.sh                                NEWDESK + SYSACC, head vs wt, on/strict/off
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"; C="$S/cards"
bash "$HERE/regress/series.sh" wt
for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$B/kernel8-head-tdq.img" "$C/tdq.img" "rg-head-$m-tdqed" "$A" 'NEVER-ENDS' 45 25 40
  bash "$HERE/regress/run.sh" "$B/kernel8-wt-inj.img" "$C/p3.img" "s1-wt-$m" "$A" 'P3S1 EMU RESULT' 90
done
bash "$HERE/a113/run-idprobe.sh" "$B/kernel8-wt-plain.img" wt "$S/a113" 45
bash "$HERE/desk-acc.sh"
