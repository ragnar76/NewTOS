#!/usr/bin/env bash
# build-e10.sh <outdir> -- build the stage-3c E10 probe: E10KID.NAP
# (sdk/examples/e10kid.c) and the driver sdk/examples/e10usr.c in three
# expectation variants (E10_MODE): E10USR.NAP (protection on, default),
# E10USRS.NAP (memprot=strict), E10USRO.NAP (memprot=off).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="${1:?outdir}"
W="$(mktemp -d "$OUT/w.XXXX")"
sh "$WS/sdk/build.sh" "$WS/sdk/examples/e10kid.c" "$OUT/E10KID.NAP" > "$W/kid.log" 2>&1 || { cat "$W/kid.log"; exit 1; }
for v in "0 E10USR" "1 E10USRS" "2 E10USRO"; do
  set -- $v
  { echo "#define E10_MODE $1"; cat "$WS/sdk/examples/e10usr.c"; } > "$W/e10_$1.c"
  sh "$WS/sdk/build.sh" "$W/e10_$1.c" "$OUT/$2.NAP" > "$W/$2.log" 2>&1 || { cat "$W/$2.log"; exit 1; }
done
grep -h -i "warning" "$W"/*.log || true
echo "built E10KID.NAP + E10USR/E10USRS/E10USRO.NAP"
rm -rf "$W"
