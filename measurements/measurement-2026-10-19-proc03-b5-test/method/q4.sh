#!/usr/bin/env bash
# q4.sh -- W scenario (b5probe-w.c): Z ends (Pterm) while the accessory sits in Pexec of a child parked in evnt_timer(4000); head vs wt, on/strict/off.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"; C="$S/cards"
for side in wt head; do for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  bash "$HERE/regress/run.sh" "$B/kernel8-$side-plain.img" "$C/b5w.img" "w-$side-$m" "$A" 'B5 ACC RESULT' 100
done; done
echo Q4-DONE
