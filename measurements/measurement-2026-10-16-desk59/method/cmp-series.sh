#!/usr/bin/env bash
# cmp-series.sh -- series2.sh results: per mode, EL0 vs desk=kernel (pixdiff, APITEST lines, hex-normalised serial diff, EXC/PANIC counts) and
# each dump vs the u9-3-5 developer reference (pixdiff). cwd-independent.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"; S="$HERE/../serial"; R="$HERE/../../measurement-2026-10-14-desk43-u9-3-5/screendumps"
norm() { grep -aE '\[(OK|FAIL|SKIP)[^]]*\]' "$1" | tr -d '\r' | sed -E 's/0x[0-9a-fA-F]+/0xX/g; s/\$[0-9a-fA-F]+/$X/g'; }
hx() { tr -d '\r' < "$1" | sed -E 's/0x[0-9a-fA-F]+/0xX/g'; }
pd() { python3 "$HERE/pixdiff.py" "$1" "$2" --boxes | tr '\n' ' '; }
for m in ${MODES:-on off strict on-asid verify}; do
  echo "=== mode $m"
  for c in desktop apitest teradesk qed; do
    for f in "$D/s-el0-$m-$c-"*.png; do b=${f#$D/s-el0-$m-}; g="$D/s-el1-$m-$b"
      echo "  EL0 vs EL1 $m $b: $(pd "$g" "$f")"
    done
    e0="$S/s-el0-$m-$c-serial.log"; e1="$S/s-el1-$m-$c-serial.log"
    echo "  $c: exc/panic/loadfail EL0=$(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$e0") EL1=$(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$e1"); serial diff lines (hex-norm) $(diff <(hx "$e1") <(hx "$e0") | grep -c '^[<>]')"
    diff <(hx "$e1") <(hx "$e0") | grep '^[<>]' | cut -c1-160 | head -8
  done
  echo "  APITEST result lines EL0 vs EL1: $(diff <(norm "$S/s-el1-$m-apitest-serial.log") <(norm "$S/s-el0-$m-apitest-serial.log") | grep -c '^[<>]') differing of $(norm "$S/s-el0-$m-apitest-serial.log" | wc -l | tr -d ' '); $(grep -aoE 'TOTAL +[0-9]+ +[0-9]+ +[0-9]+' "$S/s-el0-$m-apitest-serial.log" | tr -s ' ')"
  case $m in verify) continue;; esac
  for p in el0 el1; do
    for pr in "desktop-t u935-$m-rpi4-desktop-t25" "apitest-t-a u935-$m-rpi4-apitest-t25" "apitest-t-b u935-$m-rpi4-apitest-t40" "teradesk-t u935-$m-rpi4-teradesk-t35" "qed-t u935-$m-rpi4-qed-t30"; do set -- $pr
      ref="$R/${2/u935-/$([ $p = el1 ] && echo u935k- || echo u935-)}.png"
      echo "  $p vs dev-ref $m $1: $(pd "$ref" "$D/s-$p-$m-$1.png")"
    done
  done
done
