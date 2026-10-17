#!/usr/bin/env bash
# api-series.sh <scratch> -- (copied from measurement-2026-10-17-a112-fix) APITEST (#Z autorun) on the red (HEAD dc3388d1) and the green kernel, memprot on
# (MODES overrides), one QEMU at a time; then api-cmp.sh.
set -uo pipefail
S="${1:?scratch}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for k in red green; do
  for m in ${MODES:-on}; do
    A="serial=on"; [ $m != on ] && A="serial=on memprot=$m"
    TMO=300 "$HERE/api-run.sh" "$S/kernel8-$k.img" "$S/api/api.img" "api-$k-$m" "$A" "TOTAL +[0-9]+" 3
  done
done
"$HERE/api-cmp.sh"
