#!/usr/bin/env bash
# verify-summary.sh <serial dir>... -- MEMPROT-FAST: per run log, the verification
# mode's verdict: VERIFY MISMATCH lines, and the last "[memprot] verify:" counter
# line (checks, pages compared, mismatches, tolerated, syncs, fallbacks).
for d in "$@"; do
  for f in "$d"/*-serial.log; do
    n=$(grep -a -c "VERIFY MISMATCH" "$f")
    last=$(grep -a "\[memprot\] verify:" "$f" | tail -1 | sed -E 's/.*verify: ([0-9]+) checks, ([0-9]+) pages compared, ([0-9]+) mismatches, ([0-9]+) tolerated.*rebuilds ([0-9]+) fallbacks ([0-9]+).*/checks=\1 compared=\2 mismatches=\3 tolerated=\4 rebuilds=\5 fallbacks=\6/')
    on=$(grep -a -o "memory protection (ON|OFF)[^:]*" -E "$f" | head -1)
    printf '%-40s MISMATCH-lines=%s %s | %s\n' "$(basename "$f" -serial.log)" "$n" "${last:-no-counter-line}" "$on"
  done
done
