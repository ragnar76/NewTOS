#!/usr/bin/env bash
# cmp-api.sh -- APITEST red vs green per mode: the TOTAL line and the result lines ([OK]/[FAIL]/[SKIP] and "SKIP"),
# hex addresses normalised (0x...), compared line by line. Output to ../checks/api-red-vs-green.txt.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; SER="$HERE/../serial"; OUT="$HERE/../checks/api-red-vs-green.txt"
: > "$OUT"
norm() { grep -E '\[(OK|FAIL|SKIP)\]|SKIP|TOTAL' "$1" | sed -E 's/0x[0-9a-fA-F]+/0x_/g'; }
for m in on off strict; do
  r="$SER/api-red-$m-serial.log"; g="$SER/api-green-$m-serial.log"
  echo "mode $m: red  $(grep TOTAL "$r" | tail -1 | tr -s ' ')" >> "$OUT"
  echo "mode $m: green $(grep TOTAL "$g" | tail -1 | tr -s ' ')" >> "$OUT"
  echo "mode $m: result lines red $(norm "$r" | wc -l | tr -d ' ') green $(norm "$g" | wc -l | tr -d ' '), differing: $(diff <(norm "$r") <(norm "$g") | grep -c '^[<>]')" >> "$OUT"
  diff <(norm "$r") <(norm "$g") >> "$OUT"
  echo "mode $m: EXC/PANIC red $(grep -Ec '\[EXC\]|PANIC|panicked' "$r") green $(grep -Ec '\[EXC\]|PANIC|panicked' "$g")" >> "$OUT"
done
cat "$OUT"
