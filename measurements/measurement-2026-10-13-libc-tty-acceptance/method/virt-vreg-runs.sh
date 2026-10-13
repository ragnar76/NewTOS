#!/usr/bin/env bash
# virt-vreg-runs.sh [modes...] -- qemu-virt (no SD) runs of the RAM-registry probe kernels (vreg-kernels.sh):
#   vreg-ac      STDOUTDB, STDERRP (new crt0), FS11A, STDERRA, APITEST  (ends at APITEST's TOTAL line)
#   vreg-old-ac  STDERRP with the OLD crt0 (the hang reproduction), then STDERRA (ends after STDERRA)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$AV"
  bash "$HERE/run-virt.sh" "$B/kernel-virt-vreg-ac.img" "ac-$m-virt-vreg" --end 'TOTAL +[0-9]+ +[0-9]+ +[0-9]+' --end-delay 2 --timeout 400
  bash "$HERE/run-virt.sh" "$B/kernel-virt-vreg-old-ac.img" "ac-$m-virt-vreg-oldcrt0" --end '\[vreg\] back after STDERRA' --end-delay 2 --timeout 150
done
