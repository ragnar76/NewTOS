#!/usr/bin/env bash
# ldg-runs.sh <scratch root> [part...] -- the QEMU raspi4b runs of TASK/LDG-M1
# (serial=on). Cards: seed-card.sh (copied from measurement-2026-10-06-port09,
# private images in the scratch root, never tools/make-sd-image.sh); every
# program in ../bin goes to C:\ (the S2 probes rebuilt from this tree by
# build-s2-tests.sh, LDGTEST.NCP), EX_LIB.NLD to C:\LDG\. Kernel:
# ../bin/kernel8-ldgm1.img (checks.sh ldgm1 = the unchanged kernel of HEAD).
# Parts:
#   ldg      \AUTO\LDGTEST.NCP (before the AES), memprot=off, on, strict
#   ldgz     C:\LDGTEST.NCP launched by the desktop's #Z autorun (a GEM-side
#            parent environment), memprot on
#   neg      \AUTO\LDGTEST.NCP on a card WITHOUT C:\LDG\EX_LIB.NLD (expected:
#            "ldgtest: FAIL (4 of 7 checks failed)", no leak, boot goes on), on
#   regr     memprot on: S2ALL, APITEST, QED (#Z), TeraDesk (#Z)
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; K="$B/kernel8-ldgm1.img"; C="$S/ldgm1-cards"
mkdir -p "$C"
LDGX="LDG/EX_LIB.NLD=$B/EX_LIB.NLD"
sd() { [ -f "$C/$1.img" ] || EXTRA="$LDGX ${3:-}" bash "$HERE/seed-card.sh" "$C/$1.img" "$2" > /dev/null; }
rd() { bash "$HERE/run-rpi4-drive.sh" "$@"; }
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
END='ldgtest: (ALL PASS|FAIL \()'
for part in "$@"; do case "$part" in
ldg)
  sd ldgauto - "AUTO/LDGTEST.NCP=$B/LDGTEST.NCP"
  for M in off on strict; do
    A="serial=on"; [ $M != on ] && A="serial=on memprot=$M"
    APPEND="$A" rd "$K" "$C/ldgauto.img" "ldgm1-$M-rpi4-ldgauto" --end "$END" --end-delay 2 --timeout 240
  done
  ;;
ldgz)
  sd ldgz 'C:\LDGTEST.NCP'
  APPEND=serial=on rd "$K" "$C/ldgz.img" ldgm1-on-rpi4-ldgz --end "$END" --end-delay 2 --timeout 240
  ;;
neg)
  [ -f "$C/ldgneg.img" ] || EXTRA="AUTO/LDGTEST.NCP=$B/LDGTEST.NCP" bash "$HERE/seed-card.sh" "$C/ldgneg.img" - > /dev/null
  APPEND=serial=on rd "$K" "$C/ldgneg.img" ldgm1-on-rpi4-ldgneg-nomodule --end "$END" --end-delay 6 --timeout 240
  ;;
regr)
  export APPEND=serial=on
  sd s2all 'C:\S2ALL.NAP'; sd apitest 'C:\APITEST.NCP'
  sd qed 'C:\Apps\QED\QED.NAP'; sd teradesk 'C:\Apps\TERADESK\DESKTOP.NAP'
  rd "$K" "$C/s2all.img"    ldgm1-on-rpi4-s2all --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
  r4 "$K" "$C/apitest.img"  ldgm1-on-rpi4-apitest  25:t25.png 40:t40.png
  r4 "$K" "$C/qed.img"      ldgm1-on-rpi4-qed      30:t30.png
  r4 "$K" "$C/teradesk.img" ldgm1-on-rpi4-teradesk 35:t35.png
  ;;
*) echo "part? $part"; exit 2;;
esac; done
