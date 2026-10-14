#!/usr/bin/env bash
# build-kernels.sh <src-tree> <name> [--features F] -- <inject-arm ARM>... -- DESK43 U9-1/U9-2: one scratch rpi4 release
# kernel with the U9-1 instruments (u91-instr.py) and, when ARMs are given, the T10 injector (tools/inject-arm.py,
# tag u91). rsync <src-tree> -> /tmp/m5j/u912/tree-<name>; result -> ../bin/kernel8-<name>.img. Never the shared tree.
set -euo pipefail
SRC="${1:?}"; NAME="${2:?}"; shift 2
FEAT=()
if [ "${1:-}" = "--features" ]; then FEAT=(--features "$2"); shift 2; fi
[ "${1:-}" = "--" ] && shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"; WS="$(cd "$M/../.." && pwd)"; W=/tmp/m5j/u912
export SOURCE_DATE_EPOCH=1791979200
rsync -a --delete --exclude target "$SRC/" "$W/tree-$NAME/"
python3 "$HERE/u91-instr.py" "$W/tree-$NAME"
[ $# -gt 0 ] && python3 "$WS/tools/inject-arm.py" "$W/tree-$NAME" --tag u91 "$@"
(cd "$W/tree-$NAME" && cargo xtask build --board rpi4 --release ${FEAT[@]+"${FEAT[@]}"}) > "$W/build-$NAME.log" 2>&1 \
  || { tail -40 "$W/build-$NAME.log"; exit 1; }
echo "$NAME: $(grep -E 'generated [0-9]+ warning' "$W/build-$NAME.log" | tr '\n' ' ')"
cp "$W/tree-$NAME/kernel8.img" "$M/bin/kernel8-$NAME.img"
