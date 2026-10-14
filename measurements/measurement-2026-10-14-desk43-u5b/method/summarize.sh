#!/usr/bin/env bash
# summarize.sh -- DESK43 U5b: per run (series.sh, labels base/u5b) the verdict lines
# (desktop up, APITEST TOTAL) and the [EXC]/PANIC count; then
#   (1) screendumps u5b vs base (same mode, same card, same second),
#   (2) screendumps vs the MEM05 acceptance references (acc-<mode>-rpi4-*),
#   (3) APITEST [OK]/[FAIL]/[SKIP] lines u5b vs base and vs MEM05 (modulo hex).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
A="$HERE/../../measurement-2026-10-13-mem05-acceptance"
for f in "$S"/base-*-serial.log "$S"/u5b-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(TOTAL[^\r]*|desktop up[^\r]*)' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:' "$f")"
  echo "$t | $v | exc/panic=$e"
done
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | grep -av '^\[mm\]' | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g' ; }
echo "--- APITEST result lines (modulo hex)"
for m in on off strict; do
  a="$S/u5b-$m-rpi4-apitest-serial.log"; b="$S/base-$m-rpi4-apitest-serial.log"; c="$A/serial/acc-$m-rpi4-apitest-serial.log"
  echo "$m: u5b vs base $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing lines; u5b vs MEM05 $(diff <(norm "$c") <(norm "$a") | grep -c '^[<>]') differing lines ($(norm "$a" | wc -l | tr -d ' ') lines)"
done
echo "--- screendumps u5b vs base"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "u5b-$m-rpi4-" "$D" "base-$m-rpi4-"; done
echo "--- screendumps u5b vs MEM05 acceptance (acc-<mode>-rpi4-)"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "u5b-$m-rpi4-" "$A/screendumps" "acc-$m-rpi4-"; done
echo "--- desktop t25 sha256"
shasum -a 256 "$D"/*-desktop-t25.png | sed "s#$D/##"
