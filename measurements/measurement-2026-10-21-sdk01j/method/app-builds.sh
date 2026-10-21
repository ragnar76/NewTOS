#!/usr/bin/env bash
# app-builds.sh -- after the libc/libm change: run the six application builds with their own scripts (none links through
# newtos-cc, so the compiler-rt builtins are not on their link lines -- they must link without them), collect "LINK OK" /
# "undefined symbols" lines, then sha256 of every application binary -> ../checks/apps-after.sha256. The "before" hashes
# (../checks/apps-before.sha256) were taken from the binaries in the tree at HEAD 04f81551 before any rebuild.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; cd "$WS/sdk"
C="$HERE/../checks"; mkdir -p "$C" "$S/logs"; : > "$C/app-builds.txt"
for s in apps/build-qed.sh apps/build-teradesk.sh apps/build-coremark.sh apps/build-doom.sh zview/build-zview.sh apps/apitest/build-apitest.sh; do
  echo "== $s" >> "$C/app-builds.txt"
  "./$s" > "$S/logs/$(basename "$s" .sh).log" 2>&1; rc=$?
  echo "rc=$rc" >> "$C/app-builds.txt"
  grep -aE "LINK OK|LINK FAILED|undefined symbol|undefined symbols" "$S/logs/$(basename "$s" .sh).log" >> "$C/app-builds.txt"
done
find apps/qed/build apps/teradesk/build apps/coremark/build apps/doom/build zview/build/out zview/build/dist -maxdepth 3 \
  \( -name '*.NAP' -o -name '*.NCP' -o -name '*.NLD' \) | sort | xargs shasum -a 256 > "$C/apps-after.sha256"
shasum -a 256 apps/apitest/APITEST.NCP >> "$C/apps-after.sha256"
wc -l < "$C/apps-after.sha256"
