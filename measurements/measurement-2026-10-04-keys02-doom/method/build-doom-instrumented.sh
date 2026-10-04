#!/usr/bin/env bash
# build-doom-instrumented.sh <scratch-dir> [m68k]
# Builds DOOM from the K02-instrumented scratch copy (instrument-doom.py)
# with the unchanged sdk/apps/build-doom.sh / build-doom-m68k.sh: a copy of
# the script whose PORT points at the scratch copy, everything else (SDK,
# libraries, vendored doomgeneric) the shared tree's, read-only.
# DOOM_ARGS adds -warp 1 (straight into E1M1) to the usual zone size;
# EXTRA='"-nokbdvec"' appends more (the fallback comparison run).
set -euo pipefail
SCR="$1"; ARCH="${2:-aarch64}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
python3 "$HERE/instrument-doom.py" "$WS/sdk/apps/doom" "$SCR/src"
if [ "$ARCH" = m68k ]; then
  S="$WS/sdk/apps/build-doom-m68k.sh"; ARGS='"-mb","6","-warp","1"'${EXTRA:+,$EXTRA}; OUT="$SCR/DOOM.PRG"
else
  S="$WS/sdk/apps/build-doom.sh"; ARGS='"-mb","2","-warp","1"'${EXTRA:+,$EXTRA}; OUT="$SCR/DOOM.NAP"
fi
sed -e "s#^SDK=.*#SDK=\"$WS/sdk\"#" -e "s#^PORT=.*#PORT=\"$SCR/src\"#" "$S" > "$SCR/build.sh"
BUILD="$SCR/build" OUT="$OUT" DOOM_ARGS="$ARGS" bash "$SCR/build.sh"
