#!/usr/bin/env bash
# shots-vs-a4.sh <a4 screendumps> <mf screendumps> <mode> -- MEMPROT-FAST: byte
# comparison of every a4-<mode>-<rest>.png (measurement-2026-10-06-arch03-4)
# with mf-<mode>-<rest>.png; longer mode names under a shorter prefix
# (asid-*, tiny-*) are skipped. Prints SAME/DIFF/MISSING per file + a count.
A="$1"; B="$2"; m="$3"; same=0; diff=0; miss=0
for f in "$A"/a4-$m-*.png; do
  r=${f#$A/a4-$m-}
  case "$m:$r" in on:asid-*|on:tiny-*|off:asid-*|off:tiny-*|strict:asid-*|strict:tiny-*) continue;; esac
  g="$B/mf-$m-$r"
  if [ ! -f "$g" ]; then echo "MISSING $r"; miss=$((miss+1))
  elif cmp -s "$f" "$g"; then echo "SAME $r"; same=$((same+1))
  else echo "DIFF $r"; diff=$((diff+1)); fi
done
echo "a4-$m- vs mf-$m-: same=$same diff=$diff missing=$miss"
