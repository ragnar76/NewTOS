#!/usr/bin/env bash
# m5-virt.sh <label> [modes...] -- qemu-virt boot + selftests + desktop + crash kernel per mode
# (copy of measurement-2026-10-13-libc-tty-acceptance/method/virt-boot-runs.sh with the label as
# argument). cwd = /tmp/m5j (relative socket dir s/).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; L="${1:?}"; shift
for m in ${*:-on off strict on-asid}; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$AV"
  bash "$HERE/run-virt.sh" "$B/kernel-virt-st-$L.img" "$L-$m-virt-selftests" --timeout 240
  bash "$HERE/run-virt.sh" "$B/kernel-virt-$L.img" "$L-$m-virt-desktop" --timeout 120
  bash "$HERE/run-virt.sh" "$B/kernel-virt-crash-$L.img" "$L-$m-virt-crash" --answer --end 'E10USR: (PASS|FAIL)' --end-delay 1.5 --timeout 700
done
