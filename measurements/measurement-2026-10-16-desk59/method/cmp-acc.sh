#!/usr/bin/env bash
# cmp-acc.sh -- DESK59: every views/series dump of this run (EL0 default desktop, on/off/strict) vs the same-named dump of
# measurements/measurement-2026-10-15-desk43-acceptance (pixdiff.py --boxes), plus EXC/PANIC counts and the APITEST TOTAL line.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"; S="$HERE/../serial"
R="$HERE/../../measurement-2026-10-15-desk43-acceptance/screendumps"
for f in "$D"/views-el0-*.png "$D"/s-el0-*.png; do b=$(basename "$f")
  [ -f "$R/$b" ] || { echo "$b: NO REFERENCE"; continue; }
  echo "$b: $(python3 "$HERE/pixdiff.py" "$R/$b" "$f" --boxes | tr '\n' ' ')"
done
for l in "$S"/views-el0-*-serial.log "$S"/s-el0-*-serial.log; do
  echo "$(basename "$l"): exc/panic $(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$l") $(grep -aoE 'TOTAL +[0-9]+ +[0-9]+ +[0-9]+' "$l" | tr -s ' ')"
done
