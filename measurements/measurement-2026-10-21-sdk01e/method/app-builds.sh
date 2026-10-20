#!/usr/bin/env bash
# app-builds.sh <before|after> -- sha256 of every application binary the SDK builds; with "after" the five
# application builds are run first (their own scripts; none of them goes through newtos-cc, so the builtins
# archive is not on their link lines). Output -> ../checks/apps-<tag>.sha256 (+ build tails for "after").
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; cd "$WS/sdk"
TAG="${1:?before|after}"; C="$HERE/../checks"; mkdir -p "$C"
if [ "$TAG" = after ]; then
  : > "$C/app-builds.txt"
  for s in apps/build-qed.sh apps/build-teradesk.sh apps/build-coremark.sh apps/build-doom.sh zview/build-zview.sh; do
    echo "== $s" >> "$C/app-builds.txt"
    "./$s" > "$S/$(basename "$s" .sh).log" 2>&1; rc=$?
    echo "rc=$rc" >> "$C/app-builds.txt"
    grep -aE "LINK OK|undefined symbols" "$S/$(basename "$s" .sh).log" >> "$C/app-builds.txt"
  done
fi
find apps/qed/build apps/teradesk/build apps/coremark/build apps/doom/build zview/build/out zview/build/dist -maxdepth 3 \
  \( -name '*.NAP' -o -name '*.NCP' -o -name '*.NLD' \) | sort | xargs shasum -a 256 > "$C/apps-$TAG.sha256"
wc -l < "$C/apps-$TAG.sha256"
