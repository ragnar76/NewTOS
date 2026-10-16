#!/usr/bin/env bash
# (PROC06: copied from measurement-2026-10-16-t16-desk60/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06, kernel id p06; changes listed in RUNINFO.TXT.)
# cmp-acc.sh -- T16+DESK60 (copy of desk58 cmp-acc.sh, reference = measurement-2026-10-16-desk58): every views/series dump of this run (EL0 default desktop,
# on/off/strict) vs the same-named dump of measurements/measurement-2026-10-16-desk58 (cmp byte-equality + pixdiff.py --boxes),
# plus EXC/PANIC counts and the APITEST TOTAL line.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"; S="$HERE/../serial"
R="$HERE/../../measurement-2026-10-16-t16-desk60/screendumps"
for f in "$D"/views-el0-*.png "$D"/s-el0-*.png; do b=$(basename "$f")
  [ -f "$R/$b" ] || { echo "$b: NO REFERENCE"; continue; }
  echo "$b: $(cmp -s "$R/$b" "$f" && echo BYTE-IDENTICAL || echo BYTES-DIFFER) $(python3 "$HERE/pixdiff.py" "$R/$b" "$f" --boxes | tr '\n' ' ')"
done
for l in "$S"/views-el0-*-serial.log "$S"/s-el0-*-serial.log; do
  echo "$(basename "$l"): exc/panic $(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$l") $(grep -aoE 'TOTAL +[0-9]+ +[0-9]+ +[0-9]+' "$l" | tr -s ' ')"
done
