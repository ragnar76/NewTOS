#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <object.rs-source: tree|HEAD>
# Copies the crates into a scratch dir (never the shared working tree), applies
# the H02/A46 injector scaffolding (scratch copy only) and optionally swaps in
# HEAD's tos-api/src/aes/object.rs ("before": the tree's only product change
# for A68-A70 is object.rs).
set -euo pipefail
WS="$1"; SCR="$2"; OBJ="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ "$OBJ" = "HEAD" ]; then
  git -C "$WS" show HEAD:tos-api/src/aes/object.rs > "$SCR/tos-api/src/aes/object.rs"
  echo "object.rs = HEAD ($(git -C "$WS" rev-parse --short HEAD))"
else
  echo "object.rs = working tree"
fi
python3 "$HERE/a68_patch.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$HERE/a68_scripts.rs"
python3 "$HERE/a68_patch2.py" "$SCR"
echo "scratch ready: $SCR"
