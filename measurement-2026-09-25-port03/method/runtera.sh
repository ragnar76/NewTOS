#!/usr/bin/env bash
# runtera.sh <tree-dir> <timeout> <SPEC...>
# Like runone.sh for the tag `port03-tera`, but the card copy gets the #Z
# autorun NEWDESK.INF (seed-tera.sh) before the run.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TREE="$1"; TMO="$2"; shift 2
TAG=port03-tera
SCR="$(dirname "$TREE")"
python3 "$HERE/apply_injector.py" "$TREE" "$HERE/$TAG.rs" >/dev/null
python3 "$HERE/a78_patch2.py"     "$TREE" >/dev/null
python3 "$HERE/key_step_patch.py" "$TREE" >/dev/null
python3 "$HERE/t10_workaround.py" "$TREE" >/dev/null
( cd "$TREE" && cargo run -p xtask -- build --board rpi4 >/dev/null 2>&1 )
cp "$SCR/tmp/base.img" "$SCR/tmp/$TAG.img"
bash "$HERE/seed-tera.sh" "$SCR/tmp/$TAG.img"
bash "$HERE/run.sh" "$TREE/kernel8.img" "$SCR/tmp/$TAG.img" "$SCR/tmp/out" "$TAG" "$TMO" "$@"
