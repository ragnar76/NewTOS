#!/usr/bin/env bash
# OFAM freeze group A: the ZVIEW04 regression set on the HEAD kernel only (no head-/fix- pair).
set -uo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export SOCKROOT="$S"; KP=""; export KP
bash "$HERE/evb-runs.sh" "$S" accept desk tera qed desk3a api
for x in zv-dclick zv-browser zv-quit; do
  bash "$HERE/zv01-runs.sh" "$S" zview "$S/kinj-$x.img" "$x"
done
echo "=== group A done"
