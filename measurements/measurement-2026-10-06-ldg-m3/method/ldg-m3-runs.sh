#!/usr/bin/env bash
# ldg-m3-runs.sh <scratch root> [part...] -- the QEMU raspi4b runs of
# TASK/LDG-M3 (serial=on). Kernels: make-scratch-kernels.sh (same scratch
# root). Cards: seed-card.sh (copied from measurement-2026-10-06-ldg-m1),
# private 64 MiB images in <S>/cards, never tools/make-sd-image.sh; the demo's
# tree from ../bin/LDGDEMO goes to C:\LDGDEMO\ (CLIENT\LDGDEMO.NAP +
# CLIENT.RSC, LIBS\EX_LIB.NLD + POPUP.NLD), the #Z line starts
# C:\LDGDEMO\CLIENT\LDGDEMO.NAP.
# Parts:
#   probe   HEAD kernel, no input: screendumps at 20 s / 35 s (window up)
#   accept  run C (keyboard + long press, see m3c.actions), memprot on AND
#           strict, injector kernel kinj-m3c.img
#   popup   run B (Alt+F -> POPUP.NLD's __popup), memprot on: finding B2
#   trace   run A with the AES trace kernel (short click vs long press):
#           finding B1; C:\AESTRACE.BIN copied out to ../checks/
#   regr    HEAD kernel, memprot on: APITEST, QED (#Z), TeraDesk (#Z)
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
B="$HERE/../bin"; C="$S/cards"; mkdir -p "$C"
CHK="$HERE/../checks"; mkdir -p "$CHK"
DEMO="LDGDEMO/CLIENT/LDGDEMO.NAP=$B/LDGDEMO/CLIENT/LDGDEMO.NAP LDGDEMO/CLIENT/CLIENT.RSC=$B/LDGDEMO/CLIENT/CLIENT.RSC LDGDEMO/LIBS/EX_LIB.NLD=$B/LDGDEMO/LIBS/EX_LIB.NLD LDGDEMO/LIBS/POPUP.NLD=$B/LDGDEMO/LIBS/POPUP.NLD"
sd() { [ -f "$C/$1.img" ] || EXTRA="${3:-}" BIN="$B" bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
rd() { bash "$HERE/run-rpi4-drive-m3.sh" "$@"; }
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
PART=63   # MBR partition 1 starts at sector 63 (diskutil, 64 MiB card)
for part in "$@"; do case "$part" in
probe)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  APPEND=serial=on r4 "$S/kernel8-m3.img" "$C/ldgdemo.img" probe-on 20:t20.png 35:t35.png
  ;;
accept)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  for M in on strict; do
    A="serial=on"; [ $M != on ] && A="serial=on memprot=$M"
    APPEND="$A" rd "$S/kinj-m3c.img" "$C/ldgdemo.img" "m3c-$M" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150
  done
  ;;
popup)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  APPEND=serial=on rd "$S/kinj-m3b.img" "$C/ldgdemo.img" m3b-on --mark-delay 0.3 \
    --end '\[m3\] MARK S05_SINUS_SET' --end-delay 2 --timeout 150
  ;;
trace)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  APPEND=serial=on rd "$S/kinj-m3a-trace.img" "$C/ldgdemo.img" m3a-trace-on --mark-delay 0.3 \
    --end '\[m3\] MARK END_CLOSED' --end-delay 2 --timeout 150
  # run-rpi4-drive-m3.sh leaves the run's private card copy in place
  mcopy -n -o -i "$C/run-m3a-trace-on.img@@$((PART*512))" ::AESTRACE.BIN "$CHK/m3a-trace-AESTRACE.BIN"
  python3 "$WS/tools/aestrace.py" "$CHK/m3a-trace-AESTRACE.BIN" > "$CHK/m3a-trace-aestrace.txt"
  ;;
regr)
  export APPEND=serial=on
  sd apitest 'C:\APITEST.NCP'; sd qed 'C:\Apps\QED\QED.NAP'; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'
  r4 "$S/kernel8-m3.img" "$C/apitest.img"  m3-on-apitest  25:t25.png 40:t40.png
  r4 "$S/kernel8-m3.img" "$C/qed.img"      m3-on-qed      30:t30.png
  r4 "$S/kernel8-m3.img" "$C/teradesk.img" m3-on-teradesk 35:t35.png
  ;;
*) echo "part? $part"; exit 2;;
esac; done
