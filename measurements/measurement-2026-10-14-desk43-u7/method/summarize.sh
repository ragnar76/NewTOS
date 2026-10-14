#!/usr/bin/env bash
# summarize.sh -- DESK43 U7 (adapted from measurement-2026-10-14-desk43-u5b/method/summarize.sh):
# per run (series.sh, label u7) the verdict lines (desktop up, APITEST TOTAL) and
# the [EXC]/PANIC count; then
#   (1) screendumps u7 vs the U5b run's u5b-* dumps (same mode, card, second),
#   (2) screendumps u7 vs the U5b run's base-* dumps (HEAD f6f14885),
#   (3) APITEST [OK]/[FAIL]/[SKIP] lines u7 vs U5b (modulo hex).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
R="$HERE/../../measurement-2026-10-14-desk43-u5b"
for f in "$S"/u7-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(TOTAL[^\r]*|desktop up[^\r]*)' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:' "$f")"
  echo "$t | $v | exc/panic=$e"
done
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | grep -av '^\[mm\]' | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g' ; }
echo "--- APITEST result lines (modulo hex)"
for m in on off strict; do
  a="$S/u7-$m-rpi4-apitest-serial.log"; b="$R/serial/u5b-$m-rpi4-apitest-serial.log"
  echo "$m: u7 vs u5b $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing lines ($(norm "$a" | wc -l | tr -d ' ') lines)"
done
echo "--- screendumps u7 vs U5b run u5b-*"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "u7-$m-rpi4-" "$R/screendumps" "u5b-$m-rpi4-"; done
echo "--- screendumps u7 vs U5b run base-*"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "u7-$m-rpi4-" "$R/screendumps" "base-$m-rpi4-"; done
echo "--- desktop t25 sha256"
shasum -a 256 "$D"/*-desktop-t25.png | sed "s#$D/##"
