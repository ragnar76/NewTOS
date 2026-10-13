#!/usr/bin/env bash
# fs11-apitest.sh <label> <cards-root> [modes...] -- APITEST again after the
# first series: the cards of fs11-series.sh lacked C:\DTACHILD.NCP (T29.2
# Pexec rc=-33, T29.3 SKIP); bin/ now carries it (byte copy from
# measurement-2026-10-13-crt0-main/bin). First-series logs kept in
# serial/apitest-without-dtachild/.
set -uo pipefail
L="${1:?}"; C="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="$HERE/../bin/kernel8-$L.img"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"; D="$C/$m"; mkdir -p "$D"
  [ -f "$D/apitest.img" ] || bash "$HERE/seed-card.sh" "$D/apitest.img" 'C:\APITEST.NCP' > /dev/null
  export APPEND="$A4"
  bash "$HERE/run-rpi4.sh" "$K" "$D/apitest.img" "$L-$m-rpi4-apitest" 25:t25.png 40:t40.png
done
