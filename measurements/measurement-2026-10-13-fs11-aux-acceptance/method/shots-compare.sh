#!/usr/bin/env bash
# shots-compare.sh <label> <base-mode> <other-mode...> -- byte-compare the screendumps of <base-mode> with each other mode
cd "$(dirname "${BASH_SOURCE[0]}")/../screendumps"
L="$1"; B="$2"; shift 2
for m in "$@"; do
  s=0; d=0; miss=0; dl=""
  for f in $(ls | grep "^$L-$B-" | grep -v "^$L-$B-asid-" | grep -v "^$L-on-asid-" ); do
    n=${f#$L-$B-}; o=$L-$m-$n
    [ -f "$o" ] || { miss=$((miss+1)); continue; }
    if cmp -s "$f" "$o"; then s=$((s+1)); else d=$((d+1)); dl="$dl $n"; fi
  done
  echo "== $B vs $m: same=$s differ=$d missing=$miss"
  echo "$dl" | tr ' ' '\n' | sed -E 's/-crash[0-9]+//;s/\.png//' | sort | uniq -c | tr '\n' ';'; echo
done
