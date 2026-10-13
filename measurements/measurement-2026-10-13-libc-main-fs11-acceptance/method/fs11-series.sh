#!/usr/bin/env bash
# fs11-series.sh <label> <cards-root> [modes...] -- TASK/FS11 QEMU raspi4b runs
# per protection mode (mode.sh: on | off | strict, always serial=on):
#   FS11A     product kernel, FS11A.NAP (part A: device table, no input)
#   FS11DEV   keys kernel (fs11-kernels.sh), FS11DEV.NAP (part A + part B:
#             Fread on CON with injected keys, mintlib getchar/fgets)
#   FSEEKCON, STDOUTDB, ATEXITP (serial verdicts), APITEST (#Z, screendumps +
#   serial TOTAL line) -- the probes of measurement-2026-10-13-crt0-main
set -uo pipefail
L="${1:?}"; C="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; K="$B/kernel8-$L.img"; KK="$B/kernel8-keys-$L.img"
MODES="${*:-on off strict}"
for m in $MODES; do
  echo "=== $m $(date)"
  M="$m"; . "$HERE/mode.sh"
  T="$L-$m"; D="$C/$m"; mkdir -p "$D"
  sd() { [ -f "$D/$1.img" ] || bash "$HERE/seed-card.sh" "$D/$1.img" "$2" > /dev/null; }
  sd fs11a 'C:\FS11A.NAP'; sd fs11dev 'C:\FS11DEV.NAP'
  sd fseekcon 'C:\FSEEKCON.NAP'; sd stdoutdb 'C:\STDOUTDB.NAP'
  sd atexit 'C:\ATEXITP.NAP'; sd apitest 'C:\APITEST.NCP'
  export APPEND="$A4"
  r4() { bash "$HERE/run-rpi4.sh" "$@"; }
  rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
  rd "$K"  "$D/fs11a.img"    "$T-rpi4-fs11a"    --end 'FS11DEV: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$KK" "$D/fs11dev.img"  "$T-rpi4-fs11dev"  --end 'FS11DEV: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$K"  "$D/fseekcon.img" "$T-rpi4-fseekcon" --end 'FSEEKCON: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$K"  "$D/stdoutdb.img" "$T-rpi4-stdoutdb" --end 'DBG: DONE' --end-delay 2 --timeout 200
  rd "$K"  "$D/atexit.img"   "$T-rpi4-atexit"   --end 'ATEXIT: (PASS|FAIL)' --end-delay 2 --timeout 240
  r4 "$K"  "$D/apitest.img"  "$T-rpi4-apitest"  25:t25.png 40:t40.png
done
echo "=== done $(date)"
