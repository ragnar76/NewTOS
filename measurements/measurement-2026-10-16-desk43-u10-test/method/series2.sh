#!/usr/bin/env bash
# series2.sh <mode> [paths] -- acceptance series, marker-keyed: desktop / APITEST / TeraDesk / QED on kernel bin/kernel8-u10t.img, EL0 default
# (tag s-el0-<mode>-...) and desk=kernel (s-el1-<mode>-...). Cards /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards. One QEMU at a time.
set -uo pipefail
m="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards; K="$HERE/../bin/kernel8-u10t.img"
M="$m"; . "$HERE/mode.sh"; BASEARGS="$APPEND"
for p in ${2:-el0 el1}; do
  if [ $p = el1 ]; then export APPEND="$BASEARGS desk=kernel"; else export APPEND="$BASEARGS"; fi
  T="s-$p-$m"
  bash "$HERE/mrun.sh" "$K" $C/plain.img "$T-desktop" '\[desk\] desktop up@1=8:t.png'
  bash "$HERE/mrun.sh" "$K" $C/apitest.img "$T-apitest" 'TOTAL +[0-9]+@1=2:t-a.png' 'TOTAL +[0-9]+@1=15:t-b.png'
  bash "$HERE/mrun.sh" "$K" $C/teradesk.img "$T-teradesk" '\[launch\] EL0: slot@1=12:t.png'
  bash "$HERE/mrun.sh" "$K" $C/qed.img "$T-qed" '\[launch\] EL0: slot@1=12:t.png'
done
