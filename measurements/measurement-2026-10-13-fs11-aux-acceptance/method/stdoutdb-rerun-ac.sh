#!/usr/bin/env bash
# stdoutdb-rerun-ac.sh <cards-root> <modes...> -- STDOUTDB again on the cm cards: the first cm-series pass ran before
# bin/STDOUTDB.NAP existed (build-s2-tests.sh names it STDOUTDBG.NAP; known hazard iii of the previous acceptance).
set -uo pipefail
C="$1"; shift; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
for m in "$@"; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"
  rm -f "$C/$m/stdoutdb.img"; bash "$HERE/seed-card.sh" "$C/$m/stdoutdb.img" 'C:\STDOUTDB.NAP' > /dev/null
  bash "$HERE/run-rpi4-drive.sh" "$B/kernel8-cm.img" "$C/$m/stdoutdb.img" "cm-$m-rpi4-stdoutdb-rerun" --end 'DBG: DONE' --end-delay 2 --timeout 200
done
