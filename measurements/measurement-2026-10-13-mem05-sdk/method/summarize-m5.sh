#!/usr/bin/env bash
# summarize-m5.sh <label> -- one line per run of m5-series.sh / m5-virt.sh: verdict lines, MEM05
# stats after the program, APITEST TOTAL, [EXC]/PANIC count; then the screendump comparison
# against measurement-2026-10-13-libc-tty-acceptance (prefix tta-<mode>-rpi4- / tt-on-asid-rpi4-).
set -uo pipefail
L="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
A="$HERE/../../measurement-2026-10-13-libc-tty-acceptance"
for f in "$S"/$L-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(MDPROBE: DONE|MDPAR: (PASS|FAIL)|MDBUD: (PASS|FAIL)|S2ALL: (PASS|FAIL)|MEM03: (PASS|FAIL)|E9MEM: (PASS|FAIL)|TOTAL[^\r]*|returned NULL after [0-9]+ blocks|desktop up|E10USR: (PASS|FAIL))' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:' "$f")"
  echo "$t | $v | exc/panic=$e"
done
echo "--- MEM05 lines after programs"
grep -aH "MEM05 after program" "$S"/$L-*-serial.log | sed "s|$S/||"
echo "--- screendumps vs libc-tty-acceptance"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "$L-$m-rpi4-" "$A/screendumps" "tta-$m-rpi4-"; done
python3 "$HERE/shotcmp.py" "$D" "$L-on-asid-rpi4-" "$A/screendumps" "tt-on-asid-rpi4-"
