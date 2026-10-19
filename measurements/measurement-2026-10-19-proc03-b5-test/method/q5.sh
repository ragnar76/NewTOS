#!/usr/bin/env bash
# q5.sh -- SYSACC from the Desk menu while QED runs: up:DESKTOP arm with tdq-acc.script (F1 -> QED, then acc.script +8 s); marker-keyed on the injector's feed lines.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"; C="$S/cards"; I="$HERE/fd/inter"
read -ra NS <<< "$(python3 "$HERE/fx.py" "$HERE/tdq-acc.script" 2110 2910)"
for side in ${SIDES:-head wt}; do for m in ${MODES:-on strict off}; do
  case $m in on) A="";; strict) A="memprot=strict";; off) A="memprot=off";; esac
  export APPEND_EXTRA="$A" TMO=100 QDELAY=2
  bash "$I/run.sh" "$B/kernel8-$side-acm.img" "$C/tdqa.img" "aq-$side-$m-qed" "\\[inj\\] up:DESKTOP feed@$(python3 "$HERE/fx.py" "$HERE/tdq-acc.script" 1010)=6:qed.png" "\\[inj\\] up:DESKTOP feed@${NS[0]}=3:open.png" "\\[inj\\] up:DESKTOP feed@${NS[1]}=3:after.png"
done; done
echo Q5-DONE
