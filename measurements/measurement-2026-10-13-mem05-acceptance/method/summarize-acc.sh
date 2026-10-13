#!/usr/bin/env bash
# summarize-b.sh <label> -- one line per run of b-series.sh: verdict lines, APITEST TOTAL,
# [EXC]/PANIC count; the MEM05 stats after each program; then the screendump comparison against
# the part-A run measurement-2026-10-13-mem05-sdk (prefix m5-<mode>-rpi4-), and the APITEST
# [OK]/[FAIL]/[SKIP] lines diffed against it modulo hex. Copied from summarize-m5.sh.
set -uo pipefail
L="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
A="$HERE/../../measurement-2026-10-13-mem05-sdk"
for f in "$S"/$L-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(MDPROBE: DONE|MDSTEP: DONE|HELLOMEM: DONE|MDACC: (PASS|FAIL)|MDPAR: (PASS|FAIL)|MDBUD: (PASS|FAIL)|ACCFREE: (PASS|FAIL)|PFLAGS: (PASS|FAIL)|TSR: (PASS|FAIL)|PPARENT [A-Z]+: (PASS|FAIL)|TTYPROBE: (PASS|FAIL)|CRASHTST[^\r]{0,30}(PASS|FAIL)|UACC3A: (PASS|FAIL)|MPROT3B: (PASS|FAIL)|E10USR: (PASS|FAIL)|port10: (ALL PASS|[0-9]+ of [0-9]+ checks FAILED)|DBG: DONE|STDERR[AP]\[PAR\]: DONE|ldgtest: [A-Z ]+|S2ALL: (PASS|FAIL)|MEM03: (PASS|FAIL)|E9MEM: (PASS|FAIL)|TOTAL[^\r]*|returned NULL after [0-9]+ blocks|desktop up|VERIFIED[^\r]*)' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:' "$f")"
  echo "$t | $v | exc/panic=$e"
done
echo "--- APITEST result lines vs mem05-sdk (modulo hex)"
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | grep -av '^\[mm\]' | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g' ; }
for m in on off strict on-asid; do
  a="$S/$L-$m-rpi4-apitest-serial.log"; b="$A/serial/b-$m-rpi4-apitest-serial.log"
  [ -f "$a" ] && [ -f "$b" ] && echo "$m: $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing lines ($(norm "$a" | wc -l | tr -d ' ') lines)"
done
echo "--- screendumps vs mem05-sdk (part B)"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "$L-$m-rpi4-" "$A/screendumps" "b-$m-rpi4-"; done
python3 "$HERE/shotcmp.py" "$D" "$L-on-asid-rpi4-" "$A/screendumps" "b-on-asid-rpi4-"
