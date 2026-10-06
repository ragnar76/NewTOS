#!/usr/bin/env bash
# summarize.sh <label-prefix> -- one line of serial verdicts per run log.
P="${1:?}"; S="$(cd "$(dirname "${BASH_SOURCE[0]}")/../serial" && pwd)"
for f in "$S/$P"-*-serial.log; do
  printf '%-34s %s\n' "$(basename "$f" -serial.log)" "$(grep -a -o -E 'memory protection (ON|OFF)[^:]*|desktop up|VERIFIED via [^ ]+|TOTAL +[0-9]+ +[0-9]+ +[0-9]+|[A-Z0-9]+: (PASS|FAIL)|S2ALL: (PASS|FAIL.*)|UACC3A: (PASS|FAIL)|MPROT3B: (PASS|FAIL)|E10USR: (PASS|FAIL)|E10USR: case . [^=]*=[^ ]+ want=[^ ]+ WRONG|\[memprot\] page-table arena exhausted|port10: (ALL PASS.*|[0-9]+ of [0-9]+ checks FAILED)|=== [A-Z0-9/ ]+ (PASS|FAIL) ===|Memory violation: (private|readable|free|super) memory, [a-z]+ \([a-z ]+ fault|\[EXC\]|\[crash\] slot [0-9]+ key|accessories have issued' "$f" | sort | uniq -c | tr '\n' ';' | tr -s ' ')"
done
