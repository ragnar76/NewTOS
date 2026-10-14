#!/usr/bin/env bash
# summarize.sh -- DESK43 U8a (adapted from measurement-2026-10-14-desk43-u7/method/summarize.sh):
# per run (series.sh, label u8a) the verdict lines (desktop up, APITEST TOTAL) and
# the [EXC]/PANIC count; then
#   (1) screendumps u8a vs the U7 run's u7-* dumps (same mode, card, second),
#   (2) APITEST [OK]/[FAIL]/[SKIP] lines u8a vs U7 (modulo hex),
#   (3) the desktop-up lines old (U7) vs new (U8a) per run.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
R="$HERE/../../measurement-2026-10-14-desk43-u7"
for f in "$S"/u8a-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(TOTAL[^\r]*|desktop up[^\r]*)' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:' "$f")"
  echo "$t | $v | exc/panic=$e"
done
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | grep -av '^\[mm\]' | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g' ; }
echo "--- APITEST result lines (modulo hex)"
for m in on off strict; do
  a="$S/u8a-$m-rpi4-apitest-serial.log"; b="$R/serial/u7-$m-rpi4-apitest-serial.log"
  echo "$m: u8a vs u7 $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing lines ($(norm "$a" | wc -l | tr -d ' ') lines)"
done
echo "--- screendumps u8a vs U7 run u7-*"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "u8a-$m-rpi4-" "$R/screendumps" "u7-$m-rpi4-"; done
echo "--- desktop t25 sha256"
shasum -a 256 "$D"/u8a-*-desktop-t25.png | sed "s#$D/##"
echo "--- '[desk] desktop up' lines, U7 (desktop's own) vs U8a (kernel's)"
for m in on off strict; do
  echo "$m: u7: $(grep -a '\[desk\] desktop up' "$R/serial/u7-$m-rpi4-desktop-serial.log" | tr -d '\r' | tr '\n' ';')  u8a: $(grep -a '\[desk\] desktop up' "$S/u8a-$m-rpi4-desktop-serial.log" | tr -d '\r' | tr '\n' ';')"
done
