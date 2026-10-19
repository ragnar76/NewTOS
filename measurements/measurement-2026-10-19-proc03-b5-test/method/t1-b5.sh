#!/usr/bin/env bash
# t1-b5.sh -- task 1: B5 probe, wt (green) all modes, head (red control) all modes. One QEMU at a time.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"; C="$S/cards"
for side in wt head; do for m in on strict off; do
  case $m in on) A="serial=on";; strict) A="serial=on memprot=strict";; off) A="serial=on memprot=off";; esac
  card="$C/b5.img"; [ $m = off ] && card="$C/b5d.img"
  bash "$HERE/regress/run.sh" "$S/bin/kernel8-$side-plain.img" "$card" "b5-$side-$m" "$A" 'B5 ACC RESULT' 100
done; done
