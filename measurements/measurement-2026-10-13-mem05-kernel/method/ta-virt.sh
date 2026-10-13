#!/usr/bin/env bash
# ta-virt.sh -- qemu-virt part: boot + selftests + crash kernel + desktop, RAM-registry probes, TTYA (T1 meaningful only; no C:), per mode.
# cwd = scratchpad (relative socket dir).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
bash "$HERE/virt-boot-runs.sh" on off strict on-asid
bash "$HERE/virt-vreg-runs.sh" on off strict
for m in on off strict on-asid; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$AV"
  bash "$HERE/run-virt.sh" "$B/kernel-virt-vreg-tty.img" "ta-$m-virt-vreg-tty" --end '\[vreg\] back after TTYA.NAP' --end-delay 2 --timeout 150
done
echo VIRT DONE
