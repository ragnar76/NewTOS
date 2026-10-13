#!/usr/bin/env bash
# tty-series.sh <label> <cards-root> [modes...] -- LIBC-TTY QEMU raspi4b runs per
# protection mode (mode.sh: on | off | strict, always serial=on); run recipes
# copied from measurement-2026-10-13-fs11-aux-acceptance/method (aux-series.sh,
# stderr-matrix-aux.sh, cm-series.sh):
#   TTYA      product kernel, TTYA.NAP (T1-T3: isatty std handles / file
#             handles / Fforce'd child)
#   TTYPROBE  keys kernel (tty-kernels.sh), TTYPROBE.NAP (T1-T4: + getchar /
#             fgets on stdin with the flags as mintlib leaves them)
#   STDOUTDB, STDERRP (new crt0), STDERRA -- serial verdicts
#   desktop, TeraDesk, QED, Doom, APITEST (#Z, screendumps; APITEST TOTAL line)
set -uo pipefail
L="${1:?}"; CR="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; K="$B/kernel8-$L.img"; KK="$B/kernel8-keys-tty.img"
for m in ${*:-on off strict}; do
  echo "=== $m $(date)"
  M="$m"; . "$HERE/mode.sh"; export APPEND="$A4"
  T="tt-$m"; C="$CR/$m"; mkdir -p "$C"
  sd() { [ -f "$C/$1.img" ] || bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
  sd ttya 'C:\TTYA.NAP'; sd ttyprobe 'C:\TTYPROBE.NAP'
  sd stdoutdb 'C:\STDOUTDB.NAP'; sd serr-newcrt0 'C:\STDERRP.NAP'; sd stderra 'C:\STDERRA.NAP'
  sd plain -; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'; sd qed 'C:\Apps\QED\QED.NAP'
  sd doom 'C:\DOOM\DOOM.NAP'; sd apitest 'C:\APITEST.NCP'
  r4() { bash "$HERE/run-rpi4.sh" "$@"; }
  rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
  rd "$K"  "$C/ttya.img"     "$T-rpi4-ttya"     --end 'TTYPROBE: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$KK" "$C/ttyprobe.img" "$T-rpi4-ttyprobe" --end 'TTYPROBE: (PASS|FAIL)' --end-delay 2 --timeout 200
  rd "$K"  "$C/stdoutdb.img" "$T-rpi4-stdoutdb" --end 'DBG: DONE' --end-delay 2 --timeout 200
  rd "$K"  "$C/serr-newcrt0.img" "$T-rpi4-stderr-newcrt0" --end 'STDERRP\[PAR\]: DONE' --end-delay 3 --timeout 150
  rd "$K"  "$C/stderra.img"  "$T-rpi4-stderra"  --end 'STDERRA\[PAR\]: DONE' --end-delay 3 --timeout 150
  r4 "$K"  "$C/plain.img"    "$T-rpi4-desktop"  25:t25.png
  r4 "$K"  "$C/teradesk.img" "$T-rpi4-teradesk" 35:t35.png
  r4 "$K"  "$C/qed.img"      "$T-rpi4-qed"      30:t30.png
  r4 "$K"  "$C/doom.img"     "$T-rpi4-doom"     30:t30.png 45:t45.png
  r4 "$K"  "$C/apitest.img"  "$T-rpi4-apitest"  25:t25.png 40:t40.png
done
echo "=== done $(date)"
