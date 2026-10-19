#!/usr/bin/env bash
# series-plain.sh <v...> -- runs on the PLAIN product kernel (HEAD, fresh release rpi4 build), per libc variant v (new|old), memprot default,
# one QEMU at a time, per-run APFS clones: APITEST (ends on TOTAL), CoreMark CORE (performance) + COREVAL (validation), Doom 45 s (dumps t30/t45),
# zView via ZVLAUNCH with JPG/PNG/GIF/BMP/Degas(PI1) (dumps t40/t75). fsck + card listing of every clone before deletion.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; K="$S/bin/kernel8-plain.img"; C="$S/cards"
export FSCK=1 CARDLS=1
for v in "$@"; do
  LSDIRS="/" bash "$HERE/run-plain.sh" "$K" "$C/api-$v.img" "api-$v" "serial=on" 'TOTAL ' 240
  echo "  api-$v: $(grep -a 'TOTAL ' "$HERE/../serial/api-$v.log" | tail -1 | tr -d '\r')"
  LSDIRS="/ /AUTO" bash "$HERE/run-plain.sh" "$K" "$C/core-$v.img" "core-$v" "serial=on" 'CoreMark 1.0 : [0-9]' 400
  LSDIRS="/ /AUTO" bash "$HERE/run-plain.sh" "$K" "$C/coreval-$v.img" "coreval-$v" "serial=on" 'CoreMark 1.0 : [0-9]|Correct operation validated|Errors detected' 400
  LSDIRS="/ /DOOM" bash "$HERE/run-plain.sh" "$K" "$C/doom-$v.img" "doom-$v" "serial=on" 'NEVER-ENDS' 50 30 45
  for n in JPG PNG GIF BMP PI1; do
    LSDIRS="/" bash "$HERE/run-plain.sh" "$K" "$C/zv-$v-$n.img" "zv-$v-$n" "serial=on" 'NEVER-ENDS' 80 40 75
  done
done
