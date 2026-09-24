#!/usr/bin/env bash
# runone.sh <tree-dir> <img> <tag> <timeout> <SPEC...>
# Swap the injector script for <tag>.rs into the given scratch tree, rebuild for
# board rpi4, copy the card image and drive the run.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TREE="$1"; BASEIMG="$2"; TAG="$3"; TMO="$4"; shift 4
SCR="${SCR:?set SCR to the scratch directory}"
python3 "$HERE/apply_injector.py" "$TREE" "$HERE/$TAG.rs" >/dev/null
python3 "$HERE/a78_patch2.py"     "$TREE" >/dev/null
python3 "$HERE/key_step_patch.py" "$TREE" >/dev/null
( cd "$TREE" && cargo run -p xtask -- build --board rpi4 >/dev/null 2>&1 )
cp "$BASEIMG" "$SCR/tmp/$TAG.img"
bash "$HERE/run.sh" "$TREE/kernel8.img" "$SCR/tmp/$TAG.img" "$SCR/tmp/out" "$TAG" "$TMO" "$@"
