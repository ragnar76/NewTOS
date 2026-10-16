#!/usr/bin/env bash
# series.sh <scratch> -- the QEMU raspi4b runs of ARCH02 after fp-red-on / fp-green-on (one QEMU at a time):
#   FPPEXEC probe, green kernel, memprot=off / memprot=strict
#   APITEST (#Z autorun, rebuilt APITEST + PTERMCH + DTACHILD on the card), red and green kernel, modes on / off / strict
set -uo pipefail
S="${1:?scratch}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for m in off strict; do
  "$HERE/run.sh" "$S/bin/kernel8-green.img" "$S/fp-base.img" "fp-green-$m" "serial=on memprot=$m" "FPPEXEC: RESULT" 3
done
for k in red green; do
  for m in on off strict; do
    A="serial=on"; [ $m != on ] && A="serial=on memprot=$m"
    TMO=300 "$HERE/run.sh" "$S/bin/kernel8-$k.img" "$S/api-base.img" "api-$k-$m" "$A" "TOTAL +[0-9]+" 3
  done
done
