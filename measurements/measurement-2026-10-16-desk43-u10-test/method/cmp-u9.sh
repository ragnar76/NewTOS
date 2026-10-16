#!/usr/bin/env bash
# cmp-u9.sh [pattern] -- every screendump of this run with a same-named dump in the U9 acceptance (measurement-2026-10-15-desk43-acceptance, EL0 path)
# compared by pixdiff.py; one line per dump -> stdout. Dumps without a counterpart are listed as NOREF.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; D="$HERE/../screendumps"; R="$HERE/../../measurement-2026-10-15-desk43-acceptance/screendumps"
for f in "$D"/${1:-*}.png; do b=$(basename "$f")
  if [ -f "$R/$b" ]; then echo "$b: $(python3 "$HERE/pixdiff.py" "$R/$b" "$f" --boxes | tr '\n' ' ' | cut -c1-300)"; else echo "$b: NOREF"; fi
done
