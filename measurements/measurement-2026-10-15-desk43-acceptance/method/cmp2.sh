#!/usr/bin/env bash
# cmp2.sh <name> <mode>... -- per <name>-el0-<mode>: pixdiff EL0 vs EL1 of every screendump, serial: EXC/PANIC counts, launch/exit/rc lines, and the
# hex-normalised serial diff EL0 vs EL1 (first lines).
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"; S="$HERE/../serial"
n="$1"; shift
hx() { tr -d '\r' < "$1" | sed -E 's/0x[0-9a-fA-F]+/0xX/g'; }
for m in "$@"; do
  for f in "$D/$n-el0-$m-"*.png; do [ -f "$f" ] || continue; b=${f#$D/$n-el0-$m-}; g="$D/$n-el1-$m-$b"; [ -f "$g" ] || { echo "$n $m $b: no EL1 dump"; continue; }
    echo "$n $m $b: $(python3 "$HERE/pixdiff.py" "$g" "$f" --boxes | tr '\n' ' ' | cut -c1-200)"; done
  e0="$S/$n-el0-$m-serial.log"; e1="$S/$n-el1-$m-serial.log"
  echo "$n $m serial: EXC/PANIC/loadfail EL0=$(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$e0") EL1=$(grep -acE '\[EXC\]|PANIC|Panic:|load failed' "$e1") | rc lines EL0: $(grep -a 'exited with rc' "$e0" | tr -d '\r' | sed -E 's/.*(desktop|C:[^ ]*)[^ ]*( exited with rc=[0-9-]+)/\1\2/' | tr '\n' ';') EL1: $(grep -a 'exited with rc' "$e1" | tr -d '\r' | sed -E 's/.*(desktop|C:[^ ]*)[^ ]*( exited with rc=[0-9-]+)/\1\2/' | tr '\n' ';')"
  diff <(hx "$e1") <(hx "$e0") | grep '^[<>]' | cut -c1-150 | head -${DL:-12}
done
