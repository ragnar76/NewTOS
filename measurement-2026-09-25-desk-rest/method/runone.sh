#!/usr/bin/env bash
# runone.sh <tree-dir> <tag> <timeout> <SPEC...>
# Swap the injector script for <tag>.rs into the given scratch tree, rebuild for
# board rpi4, copy a fresh card image and drive the run.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TREE="$1"; TAG="$2"; TMO="$3"; shift 3
SCR="$(dirname "$TREE")"
python3 "$HERE/apply_injector.py"  "$TREE" "$HERE/$TAG.rs" >/dev/null
python3 "$HERE/a78_patch2.py"      "$TREE" >/dev/null
python3 "$HERE/key_step_patch.py"  "$TREE" >/dev/null
python3 "$HERE/t10_workaround.py"  "$TREE" >/dev/null
( cd "$TREE" && cargo run -p xtask -- build --board rpi4 >/dev/null 2>&1 )
cp "$SCR/tmp/${BASEIMG:-base}.img" "$SCR/tmp/$TAG.img"
bash "$HERE/run.sh" "$TREE/kernel8.img" "$SCR/tmp/$TAG.img" "$SCR/tmp/out" "$TAG" "$TMO" "$@"
