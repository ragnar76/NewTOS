#!/usr/bin/env bash
# compare-keyowner.sh <part...> -- screendumps of head-<part>-* against
# new-<part>-* (shotdiff.py: identical, or the differing regions; <= 20x20 px
# flagged CURSOR?, else OTHER -- every OTHER is looked at by hand) into
# ../checks/shotdiff-<part>.txt; one summary line per part.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
PY="$WS/.venv/bin/python3"
SD="$HERE/../screendumps"; CHK="$HERE/../checks"
for p in "$@"; do
  "$PY" "$HERE/shotdiff.py" "$SD" "head-$p-" "$SD" "new-$p-" > "$CHK/shotdiff-$p.txt" 2>&1
  echo "$p: $(tail -1 "$CHK/shotdiff-$p.txt")"
done
