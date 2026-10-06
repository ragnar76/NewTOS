#!/usr/bin/env bash
# build-mprot.sh <outdir> -- build the stage-3b memory-protection probe:
# MPKID.NAP (sdk/examples/mpkid.c) and the driver sdk/examples/mprot3b.c in
# three expectation variants (MPROT_MODE): MPROT3B.NAP (protection on,
# default), MPROTS.NAP (memprot=strict), MPROTO.NAP (memprot=off).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="${1:?outdir}"
W="$(mktemp -d "$OUT/w.XXXX")"
sh "$WS/sdk/build.sh" "$WS/sdk/examples/mpkid.c" "$OUT/MPKID.NAP" > "$W/mpkid.log" 2>&1 || { cat "$W/mpkid.log"; exit 1; }
for v in "0 MPROT3B" "1 MPROTS" "2 MPROTO"; do
  set -- $v
  { echo "#define MPROT_MODE $1"; cat "$WS/sdk/examples/mprot3b.c"; } > "$W/mprot_$1.c"
  sh "$WS/sdk/build.sh" "$W/mprot_$1.c" "$OUT/$2.NAP" > "$W/$2.log" 2>&1 || { cat "$W/$2.log"; exit 1; }
done
grep -h -i "warning" "$W"/*.log || true
echo "built MPKID.NAP + MPROT3B/MPROTS/MPROTO.NAP"
rm -rf "$W"
