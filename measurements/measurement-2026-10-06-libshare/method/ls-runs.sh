#!/usr/bin/env bash
# ls-runs.sh <scratch root> [part...] -- QEMU raspi4b runs of TASK/LDG-M3-B2
# (libshare in every .NLD). Kernels: make-kernels.sh (same scratch root; all
# HEAD's kernel, the injector/instrumentation only in scratch copies).
# Cards: seed-card.sh (copied unchanged from measurement-2026-10-06-ldg-m3),
# private 64 MiB images in <S>/cards, never tools/make-sd-image.sh; programs
# from ../bin (this run's build: libshare-linked EX_LIB.NLD/POPUP.NLD, the
# byte-identical clients LDGTEST.NCP/LDGDEMO.NAP).
# Parts:
#   ldg     \AUTO\LDGTEST.NCP (69 checks), memprot=off, on, strict
#   popup   LDGDEMO.NAP driven by m3b.actions (Alt+F -> POPUP.NLD __popup,
#           pick Sinus, compute, again for Cosinus, close): on, strict, off
#   accept  m3c.actions (the M3 acceptance script), on and strict
#   accept2 evb-acc.actions (the post-B1 acceptance script of
#           measurement-2026-10-06-aes-evbutton), on and strict
#   instr   m3b.actions on the instrumented kernel (Malloc/Mxalloc/Pterm
#           prints), memprot on; symbolized to ../checks/
#   regr    HEAD kernel, memprot on: APITEST, QED (#Z), TeraDesk (#Z)
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; C="$S/cards"; mkdir -p "$C"
CHK="$HERE/../checks"; mkdir -p "$CHK"
DEMO="LDGDEMO/CLIENT/LDGDEMO.NAP=$B/LDGDEMO/CLIENT/LDGDEMO.NAP LDGDEMO/CLIENT/CLIENT.RSC=$B/LDGDEMO/CLIENT/CLIENT.RSC LDGDEMO/LIBS/EX_LIB.NLD=$B/LDGDEMO/LIBS/EX_LIB.NLD LDGDEMO/LIBS/POPUP.NLD=$B/LDGDEMO/LIBS/POPUP.NLD"
sd() { [ -f "$C/$1.img" ] || EXTRA="${3:-}" BIN="$B" bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
rd() { bash "$HERE/run-rpi4-drive-m3.sh" "$@"; }
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
for part in "$@"; do case "$part" in
ldg)
  sd ldgauto - "LDG/EX_LIB.NLD=$B/EX_LIB.NLD AUTO/LDGTEST.NCP=$B/LDGTEST.NCP"
  for M in off on strict; do
    A="serial=on"; [ $M != on ] && A="serial=on memprot=$M"
    APPEND="$A" rd "$S/kernel8-ls.img" "$C/ldgauto.img" "ls-$M-ldgauto" \
      --end 'ldgtest: (ALL PASS|FAIL \()' --end-delay 2 --timeout 240
  done
  ;;
popup)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  for M in on strict off; do
    A="serial=on"; [ $M != on ] && A="serial=on memprot=$M"
    APPEND="$A" rd "$S/kinj-m3b.img" "$C/ldgdemo.img" "ls-popup-$M" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 3 --timeout 150
  done
  ;;
accept)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  for M in on strict; do
    A="serial=on"; [ $M != on ] && A="serial=on memprot=$M"
    APPEND="$A" rd "$S/kinj-m3c.img" "$C/ldgdemo.img" "ls-accept-$M" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 3 --timeout 150
  done
  ;;
accept2)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  for M in on strict; do
    A="serial=on"; [ $M != on ] && A="serial=on memprot=$M"
    APPEND="$A" rd "$S/kinj-evb-acc.img" "$C/ldgdemo.img" "ls-acc2-$M" --mark-delay 0.3 \
      --end '\[m3\] MARK END_CLOSED' --end-delay 3 --timeout 150
  done
  ;;
instr)
  sd ldgdemo 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' "$DEMO"
  APPEND=serial=on rd "$S/kinj-ls-instr.img" "$C/ldgdemo.img" ls-instr-on --mark-delay 0.3 \
    --end '\[m3\] MARK END_CLOSED' --end-delay 3 --timeout 150
  python3 "$HERE/symbolize-b2.py" "$HERE/../serial/ls-instr-on-serial.log" > "$CHK/ls-instr-on-callchains.txt"
  ;;
regr)
  export APPEND=serial=on
  sd apitest 'C:\APITEST.NCP'; sd qed 'C:\Apps\QED\QED.NAP'; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'
  r4 "$S/kernel8-ls.img" "$C/apitest.img"  ls-on-apitest  25:t25.png 40:t40.png
  r4 "$S/kernel8-ls.img" "$C/qed.img"      ls-on-qed      30:t30.png
  r4 "$S/kernel8-ls.img" "$C/teradesk.img" ls-on-teradesk 35:t35.png
  ;;
*) echo "part? $part"; exit 2;;
esac; done
