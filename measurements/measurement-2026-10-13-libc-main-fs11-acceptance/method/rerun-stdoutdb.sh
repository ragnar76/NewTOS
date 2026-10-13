#!/usr/bin/env bash
# rerun-stdoutdb.sh <cm-cards> <f11-cards> <modes...> -- STDOUTDB runs again: the first series
# found no C:\STDOUTDB.NAP (build-s2-tests.sh names it STDOUTDBG.NAP, 9 chars; renamed in bin/ now).
set -uo pipefail
CC="$1"; CF="$2"; shift 2; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"
for m in "$@"; do
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"
  for pair in "cm:$CC:kernel8-cm.img" "f11:$CF:kernel8-f11.img"; do
    IFS=: read L C K <<< "$pair"; [ -d "$C/$m" ] || continue
    rm -f "$C/$m/stdoutdb.img"; bash "$HERE/seed-card.sh" "$C/$m/stdoutdb.img" 'C:\STDOUTDB.NAP' > /dev/null
    bash "$HERE/run-rpi4-drive.sh" "$B/$K" "$C/$m/stdoutdb.img" "$L-$m-rpi4-stdoutdb" --end 'DBG: DONE' --end-delay 2 --timeout 200
  done
done
