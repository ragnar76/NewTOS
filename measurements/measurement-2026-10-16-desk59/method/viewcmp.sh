#!/usr/bin/env bash
# viewcmp.sh <prefix> <mode> -- pixel compare (pixdiff.py) of <prefix>-el0-<mode>-* vs <prefix>-el1-<mode>-* screendumps
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"
for f in "$D/$1-el0-$2-"*.png; do b=${f#$D/$1-el0-$2-}; g="$D/$1-el1-$2-$b"; [ -f "$g" ] || { echo "$b: EL1 MISSING"; continue; }
  r=$(python3 "$HERE/pixdiff.py" "$g" "$f" --boxes | tr '\n' ' '); echo "$2 $b: $r"; done
