#!/usr/bin/env bash
# summarize.sh -- DESK43 U9-3/4/5 (pattern of measurement-2026-10-14-desk43-u9-1-2/method/summarize.sh). Series runs:
#   acc  = default boot (EL0 NEWDESK), acck = desk=kernel (the built-in EL1 desktop), same kernel bin/kernel8-acc.img;
#   reference u912 = measurement-2026-10-14-desk43-u9-1-2 (= U9-0 = U8b behaviour), modes on/off/strict (no on-asid there).
# (0) per run: verdict lines (desktop up, APITEST TOTAL), [EXC]/PANIC count, desktop path line;
# (1) desk=kernel vs U9-1-2: screendumps, APITEST lines, serial logs (hex-normalised; the new boot line and the kernel
#     size line are the expected differences);
# (2) EL0 vs desk=kernel in this run: screendumps (pixdiff boxes), APITEST lines, serial (hex-normalised).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; S="$HERE/../serial"; D="$HERE/../screendumps"
R="$HERE/../../measurement-2026-10-14-desk43-u9-1-2"
for f in "$S"/acc-*-rpi4-*-serial.log "$S"/acck-*-rpi4-*-serial.log; do
  t="$(basename "$f" -serial.log)"
  v="$(grep -aoE '(TOTAL[^\r]*|desktop up[^\r]*)' "$f" | sort | uniq -c | tr -s ' ' | tr '\n' ';')"
  e="$(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$f")"
  p="$(grep -ac '\[launch\] EL0: slot' "$f")"
  echo "$t | $v | exc/panic/loadfail=$e | launches=$p"
done
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | grep -av '^\[mm\]' | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g' ; }
hx() { tr -d '\r' < "$1" | sed -E 's/0x[0-9a-fA-F]+/0xX/g'; }
echo "=== (1) desk=kernel (acck) vs U9-1-2 (u912)"
for m in on off strict; do
  python3 "$HERE/shotcmp.py" "$D" "acck-$m-rpi4-" "$R/screendumps" "u912-$m-rpi4-"
  a="$S/acck-$m-rpi4-apitest-serial.log"; b="$R/serial/u912-$m-rpi4-apitest-serial.log"
  echo "$m APITEST lines: $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing ($(norm "$a" | wc -l | tr -d ' ') lines)"
  for c in desktop apitest teradesk qed; do
    echo "-- $m $c serial diff (u912 < > acck)"; diff <(hx "$R/serial/u912-$m-rpi4-$c-serial.log") <(hx "$S/acck-$m-rpi4-$c-serial.log")
  done
done
echo "=== (2) EL0 (acc) vs desk=kernel (acck)"
for m in on off strict on-asid; do
  python3 "$HERE/shotcmp.py" "$D" "acc-$m-rpi4-" "$D" "acck-$m-rpi4-"
  for f in "$D"/acc-$m-rpi4-*.png; do
    g="$D/acck-${f#$D/acc-}"; cmp -s "$f" "$g" || echo "   $(basename "$f"): $(python3 "$HERE/pixdiff.py" "$g" "$f" --boxes | tr '\n' ' ')"
  done
  a="$S/acc-$m-rpi4-apitest-serial.log"; b="$S/acck-$m-rpi4-apitest-serial.log"
  echo "$m APITEST lines EL0 vs EL1: $(diff <(norm "$b") <(norm "$a") | grep -c '^[<>]') differing"
  for c in desktop apitest teradesk qed; do
    echo "-- $m $c serial diff (acck < > acc)"; diff <(hx "$S/acck-$m-rpi4-$c-serial.log") <(hx "$S/acc-$m-rpi4-$c-serial.log")
  done
done
