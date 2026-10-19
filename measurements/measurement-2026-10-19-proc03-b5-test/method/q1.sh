#!/usr/bin/env bash
# q1.sh -- non-interactive regression batch, sequential (one QEMU at a time): series wt+head (APITEST, S2ALL/S2ALLS, VFORKT, TeraDesk->F1->QED),
# PROC03 s1 probe head+wt on/strict/off, A113 idprobe head+wt, A112 wake probes head+wt, NEWDESK+SYSACC head/wt.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"; C="$S/cards"
bash "$HERE/regress/series.sh" wt; bash "$HERE/regress/series.sh" head
for side in wt head; do for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$B/kernel8-$side-inj.img" "$C/p3.img" "s1-$side-$m" "$A" 'P3S1 EMU RESULT' 90
done; done
for side in wt head; do bash "$HERE/a113/run-idprobe.sh" "$B/kernel8-$side-plain.img" $side "$S/a113" 45; done
for side in wt head; do bash "$HERE/a112/run-all.sh" "$B/kernel8-$side-plain.img" $side "$S/a112"; done
bash "$HERE/desk-acc.sh"
echo Q1-DONE
