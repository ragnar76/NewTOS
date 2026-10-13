#!/usr/bin/env bash
# cm-batch2.sh <label> <cards-root> [modes...] -- after cm-series.sh: S2ALL
# (strict: S2ALLS on the crash kernel) with the MEM03 child built from
# tests/envkidm.c (bin/m/ENVKID.NAP, build-envkidm.sh). envkidj.c (S2ALLJ in the
# series) expects getenv("PATH") == "C:\", which mintlib's _main rewrites to
# "/dev/c/" (mintlib/main.c:268-288) -- a stale expectation, as the a13 run
# found for envkid.c after PORT09. Recipe of a13 batch2.sh (d).
set -uo pipefail
L="${1:?}"; CR="${2:?}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; BM="$B/m"
for m in ${*:-on off strict}; do
  M="$m"; . "$HERE/mode.sh"
  T="$L-$m"; C="$CR/$m"; mkdir -p "$C"
  S2=s2all; K="$B/kernel8-$L.img"; [ "$BASE" = strict ] && { S2=s2alls; K="$B/kernel8-crash-$L.img"; }
  [ -f "$C/${S2}m.img" ] || EXTRA="ENVKID.NAP=$BM/ENVKID.NAP" bash "$HERE/seed-card.sh" "$C/${S2}m.img" "C:\\$(echo $S2 | tr a-z A-Z).NAP" > /dev/null
  bash "$HERE/run-rpi4-drive.sh" "$K" "$C/${S2}m.img" "$T-rpi4-${S2}m" --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
done
echo "=== batch2 done $(date)"
