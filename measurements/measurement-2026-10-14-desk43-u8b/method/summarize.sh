#!/usr/bin/env bash
# summarize.sh -- DESK43 U8b (adapted from measurement-2026-10-14-desk43-u6b/method/summarize.sh: u6b -> u8b, reference U8a -> U6b):
# per run (series.sh, label u8b) the verdict lines (desktop up, APITEST TOTAL) and
# the [EXC]/PANIC count; then
#   (1) screendumps u8b vs the U6b run's u6b-* dumps (same mode, card, second),
#   (2) APITEST [OK]/[FAIL]/[SKIP] lines u8b vs U6b (modulo hex),
#   (3) the desktop serial logs u8b vs U6b, hex-normalised.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
R="$HERE/../../measurement-2026-10-14-desk43-u6b"
for f in "$S"/u8b-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(TOTAL[^\r]*|desktop up[^\r]*)' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:' "$f")"
  echo "$t | $v | exc/panic=$e"
done
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | grep -av '^\[mm\]' | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g' ; }
echo "--- APITEST result lines (modulo hex)"
for m in on off strict; do
  a="$S/u8b-$m-rpi4-apitest-serial.log"; b="$R/serial/u6b-$m-rpi4-apitest-serial.log"
  echo "$m: u8b vs u6b $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing lines ($(norm "$a" | wc -l | tr -d ' ') lines)"
done
echo "--- screendumps u8b vs U6b run u6b-*"
for m in on off strict; do python3 "$HERE/shotcmp.py" "$D" "u8b-$m-rpi4-" "$R/screendumps" "u6b-$m-rpi4-"; done
echo "--- desktop t25 sha256"
shasum -a 256 "$D"/u8b-*-desktop-t25.png | sed "s#$D/##"
echo "--- desktop serial log u8b vs U6b (hex normalised, CR stripped)"
hx() { tr -d '\r' < "$1" | sed -E 's/0x[0-9a-fA-F]+/0xX/g'; }
for m in on off strict; do
  echo "== $m"; diff <(hx "$R/serial/u6b-$m-rpi4-desktop-serial.log") <(hx "$S/u8b-$m-rpi4-desktop-serial.log")
done
