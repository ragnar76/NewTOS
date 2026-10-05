#!/usr/bin/env bash
# build-probes.sh <outdir> [cases] -- build the stage-3a bad-pointer probe:
# UAKID.NAP (sdk/examples/uakid.c, one bad pointer per case) and UACC3A.NAP
# (sdk/examples/uacc3a.c, the parent driver). <cases> limits the parent's case
# list (UACC3A_CASES) to the sites converted so far; default: all.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="${1:?outdir}"; CASES="${2:-}"
W="$(mktemp -d "$OUT/w.XXXX")"
sh "$WS/sdk/build.sh" "$WS/sdk/examples/uakid.c" "$OUT/UAKID.NAP" > "$W/uakid.log" 2>&1 || { cat "$W/uakid.log"; exit 1; }
if [ -n "$CASES" ]; then
  { echo "#define UACC3A_CASES \"$CASES\""; cat "$WS/sdk/examples/uacc3a.c"; } > "$W/uacc3a.c"
else
  cp "$WS/sdk/examples/uacc3a.c" "$W/uacc3a.c"
fi
sh "$WS/sdk/build.sh" "$W/uacc3a.c" "$OUT/UACC3A.NAP" > "$W/uacc3a.log" 2>&1 || { cat "$W/uacc3a.log"; exit 1; }
grep -h -i "warning" "$W"/*.log || true
echo "built UAKID.NAP + UACC3A.NAP (cases: ${CASES:-all})"
rm -rf "$W"
