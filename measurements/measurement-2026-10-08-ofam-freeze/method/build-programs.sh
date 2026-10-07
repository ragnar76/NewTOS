#!/usr/bin/env bash
# OFAM freeze: rebuild the card programs from the tree (libc cascade via the app build scripts).
set -uo pipefail
WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"; cd "$WS"
L="${1:?log dir}"; BIN="${2:?out dir}"; mkdir -p "$L" "$BIN"
for s in apps/build-qed.sh apps/build-teradesk.sh apps/build-doom.sh apps/build-coremark.sh apps/apitest/build-apitest.sh zview/build-zview.sh; do
  bash "sdk/$s" > "$L/build-$(basename $s .sh).log" 2>&1 && echo "built via $s" || { echo "FAIL $s"; tail -20 "$L/build-$(basename $s .sh).log"; }
done
sh sdk/build.sh sdk/examples/sysacc.c "$BIN/SYSACC.NAC" > "$L/build-sysacc.log" 2>&1 && echo "built SYSACC.NAC" || echo "FAIL sysacc"
echo done
