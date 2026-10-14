#!/usr/bin/env bash
# summarize.sh -- DESK43 U9-0 (adapted from measurement-2026-10-14-desk43-u8b/method/summarize.sh: u8b -> u90, reference U6b -> U8b):
# per run (series.sh, label u90) the verdict lines (desktop up, APITEST TOTAL) and
# the [EXC]/PANIC count; then
#   (1) screendumps u90 vs the U8b run's u8b-* dumps (same mode, card, second),
#   (2) APITEST [OK]/[FAIL]/[SKIP] lines u90 vs U8b (modulo hex),
#   (3) the desktop serial logs u90 vs U8b, hex-normalised.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
R="$HERE/../../measurement-2026-10-14-desk43-u8b"
for f in "$S"/u90-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(TOTAL[^\r]*|desktop up[^\r]*)' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:' "$f")"
  echo "$t | $v | exc/panic=$e"
done
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | grep -av '^\[mm\]' | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g' ; }
echo "--- APITEST result lines (modulo hex)"
for m in on off strict; do
  a="$S/u90-$m-rpi4-apitest-serial.log"; b="$R/serial/u8b-$m-rpi4-apitest-serial.log"
  echo "$m: u90 vs u8b $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing lines ($(norm "$a" | wc -l | tr -d ' ') lines)"
done
echo "--- screendumps u90 vs U8b run u8b-*"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "u90-$m-rpi4-" "$R/screendumps" "u8b-$m-rpi4-"; done
echo "--- desktop t25 sha256"
shasum -a 256 "$D"/u90-*-desktop-t25.png | sed "s#$D/##"
echo "--- desktop serial log u90 vs U8b (hex normalised, CR stripped)"
hx() { tr -d '\r' < "$1" | sed -E 's/0x[0-9a-fA-F]+/0xX/g'; }
for m in on off strict; do
  echo "== $m"; diff <(hx "$R/serial/u8b-$m-rpi4-desktop-serial.log") <(hx "$S/u90-$m-rpi4-desktop-serial.log")
done
