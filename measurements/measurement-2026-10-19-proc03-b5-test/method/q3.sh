#!/usr/bin/env bash
# q3.sh -- SYSACC opened from the Desk menu while QED runs (TeraDesk F1 -> QED, armed on launch:QED, acc-late.script), head vs wt, on/strict/off.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"; C="$S/cards"; I="$HERE/fd/inter"
read -ra NS <<< "$(python3 "$HERE/fx.py" "$HERE/acc-late.script" 910 1910)"
for side in ${SIDES:-head wt}; do for m in ${MODES:-on strict off}; do
  case $m in on) A="";; strict) A="memprot=strict";; off) A="memprot=off";; esac
  export APPEND_EXTRA="$A" TMO=150 QDELAY=2
  bash "$I/run.sh" "$B/kernel8-$side-acl.img" "$C/tdqa.img" "al-$side-$m-qed" '\[inj\] launch:QED armed@1=4:app.png' "\\[inj\\] launch:QED feed@${NS[0]}=3:open.png" "\\[inj\\] launch:QED feed@${NS[1]}=3:after.png"
done; done
echo Q3-DONE
