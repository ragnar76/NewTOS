#!/usr/bin/env bash
# ZVIEW04: the whole regression + acceptance set, serially (one QEMU at a time),
# first on the HEAD kernels (head-), then on this change's (fix-).
#   evb-runs.sh: accept (LDGDEMO on/strict), desk, tera, qed, desk3a, api
#   zv01-runs.sh zview: zv-dclick (ZVIEW02 acceptance), zv-browser (ZVIEW01),
#                       zv-quit (this ticket's acceptance)
set -uo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export SOCKROOT="$S"
for KP in head- fix-; do
  KP=$KP bash "$HERE/evb-runs.sh" "$S" accept desk tera qed desk3a api
  for x in zv-dclick zv-browser zv-quit; do
    bash "$HERE/zv01-runs.sh" "$S" zview "$S/${KP}kinj-$x.img" "${KP}$x"
  done
done
