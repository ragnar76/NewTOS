#!/usr/bin/env bash
# build-asid.sh <outdir> -- build the ARCH03 stage-4 ASID probes from
# method/tests: ASIDKID.NAP (child), ASIDACC.NAC (accessory), and the driver
# asidstr.c in four variants: ASIDSTR.NAP (part C expects protection on or
# strict), ASIDSTRO.NAP (memprot=off), ASIDPERF.NAP / ASIDPRFO.NAP (no part C,
# more iterations -- for the product kernel and the switch-cost kernel).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="${1:?outdir}"
W="$(mktemp -d "$OUT/w.XXXX")"
cp "$HERE/tests/asidcommon.h" "$W/"
b() { # b <src in W> <out name>
  sh "$WS/sdk/build.sh" "$W/$1" "$OUT/$2" > "$W/$2.log" 2>&1 || { cat "$W/$2.log"; exit 1; }
}
cp "$HERE/tests/asidkid.c" "$HERE/tests/asidacc.c" "$W/"
b asidkid.c ASIDKID.NAP
b asidacc.c ASIDACC.NAC
for v in "0 0 ASIDSTR" "1 0 ASIDSTRO" "0 1 ASIDPERF" "1 1 ASIDPRFO"; do
  set -- $v
  { echo "#define ASID_OFF $1"; echo "#define ASIDPERF $2"; cat "$HERE/tests/asidstr.c"; } > "$W/asidstr_$3.c"
  b "asidstr_$3.c" "$3.NAP"
done
grep -h -i "warning" "$W"/*.log || true
echo "built ASIDKID.NAP ASIDACC.NAC ASIDSTR/ASIDSTRO/ASIDPERF/ASIDPRFO.NAP"
rm -rf "$W"
