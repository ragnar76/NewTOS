#!/usr/bin/env bash
# series-p1.sh <cards-root> <scratch-root> -- the full post-E9 QEMU series of
# checkpoint p1 in all three protection modes, then desk3a in each mode.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
C="${1:?}"; R="${2:?}"
for m in on off strict; do
  echo "=== $m $(date)"
  bash "$HERE/runs-poste9.sh" p1 "$C/$m" "$m"
  bash "$HERE/desk3a-run.sh" p1 "$R" "$m"
done
echo "=== done $(date)"
