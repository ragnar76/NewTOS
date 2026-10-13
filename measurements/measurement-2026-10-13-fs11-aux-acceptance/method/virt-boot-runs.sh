#!/usr/bin/env bash
# virt-boot-runs.sh [modes...] -- qemu-virt boot + selftests + crash kernel + desktop per mode (the virt part of runs-e9.sh).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; L=cm
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$AV"
  bash "$HERE/run-virt.sh" "$B/kernel-virt-st-$L.img" "$L-$m-virt-selftests" --timeout 240
  bash "$HERE/run-virt.sh" "$B/kernel-virt-$L.img" "$L-$m-virt-desktop" --timeout 120
  bash "$HERE/run-virt.sh" "$B/kernel-virt-crash-$L.img" "$L-$m-virt-crash" --answer --end 'E10USR: (PASS|FAIL)' --end-delay 1.5 --timeout 700
done
