#!/usr/bin/env bash
# aux-series.sh <label> <cards-root> [modes...] -- FS11-AUX QEMU raspi4b runs per
# protection mode (mode.sh: on | off | strict, always serial=on). Kernels:
#   bin/kernel8-<label>.img       product kernel with the AUX "no cable" entry
#   bin/kernel8-keys-<label>.img  same + KEYS02 injector + FS11 key schedule
#   bin/kernel8-pre.img           byte copy of the acceptance run's kernel8-lm.img
#                                 (fb84e64a, AUX absent) -- red controls only
# Runs:
#   fs11a / fs11dev      FS11A.NAP / FS11DEV.NAP (AUX expectations revised, W28)
#   stdoutdb             STDOUTDB.NAP
#   apitest              APITEST.NCP (#Z), screendumps t25/t40 + serial TOTAL
#   stderr-oldcrt0       the hang reproduction of the acceptance run (old crt0
#                        STDERRP, handle 2 = AUX, no Fforce) -- must complete
#   stderr-newcrt0       new crt0 STDERRP (regression)
#   stderra              new crt0, kid started with ARGV, handle 2 = AUX
#   pre-*                red controls on kernel8-pre.img (expected to hang;
#                        bounded by --timeout)
set -uo pipefail
L="${1:?}"; C="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; K="$B/kernel8-$L.img"; KK="$B/kernel8-keys-$L.img"; KP="$B/kernel8-pre.img"
for m in ${*:-on off strict}; do
  echo "=== $m $(date)"
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"
  T="$L-$m"; D="$C/$m"; mkdir -p "$D"
  sd() { [ -f "$D/$1.img" ] || bash "$HERE/seed-card.sh" "$D/$1.img" "$2" > /dev/null; }
  sd fs11a 'C:\FS11A.NAP'; sd fs11dev 'C:\FS11DEV.NAP'; sd stdoutdb 'C:\STDOUTDB.NAP'
  sd apitest 'C:\APITEST.NCP'; sd serr-newcrt0 'C:\STDERRP.NAP'; sd stderra 'C:\STDERRA.NAP'
  [ -f "$D/serr-oldcrt0.img" ] || EXTRA="STDERRP.NAP=$B/oldcrt0/STDERRP.NAP" bash "$HERE/seed-card.sh" "$D/serr-oldcrt0.img" 'C:\STDERRP.NAP' > /dev/null
  rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
  rd "$K"  "$D/fs11a.img"        "$T-rpi4-fs11a"          --end 'FS11DEV: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$KK" "$D/fs11dev.img"      "$T-rpi4-fs11dev"        --end 'FS11DEV: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$K"  "$D/stdoutdb.img"     "$T-rpi4-stdoutdb"       --end 'DBG: DONE' --end-delay 2 --timeout 200
  rd "$K"  "$D/serr-oldcrt0.img" "$T-rpi4-stderr-oldcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
  rd "$K"  "$D/serr-newcrt0.img" "$T-rpi4-stderr-newcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
  rd "$K"  "$D/stderra.img"      "$T-rpi4-stderra"        --end 'STDERRA\[PAR\]: DONE' --end-delay 3 --timeout 150
  bash "$HERE/run-rpi4.sh" "$K" "$D/apitest.img" "$T-rpi4-apitest" 25:t25.png 40:t40.png
  if [ "${PRE:-0}" = 1 ]; then
    rd "$KP" "$D/stderra.img"      "pre-$m-rpi4-stderra"        --end 'STDERRA\[PAR\]: DONE' --end-delay 3 --timeout 90
    rd "$KP" "$D/serr-oldcrt0.img" "pre-$m-rpi4-stderr-oldcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 90
  fi
done
echo "=== done $(date)"
