#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <menu.rs+event.rs source: tree|HEAD>
# Copies the crates into a scratch dir (never the shared working tree), applies
# the H02/A46 injector scaffolding + the A73 serial trace (scratch copy only)
# and optionally swaps in HEAD's tos-api/src/aes/{menu,event}.rs ("before").
set -euo pipefail
WS="$1"; SCR="$2"; SRC="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ "$SRC" = "HEAD" ]; then
  git -C "$WS" show HEAD:tos-api/src/aes/menu.rs > "$SCR/tos-api/src/aes/menu.rs"
  git -C "$WS" show HEAD:tos-api/src/aes/event.rs > "$SCR/tos-api/src/aes/event.rs"
  echo "menu.rs + event.rs = HEAD ($(git -C "$WS" rev-parse --short HEAD))"
else
  echo "menu.rs + event.rs = working tree"
fi
python3 "$HERE/a68_patch.py" "$SCR"
python3 "$HERE/a73_trace.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$HERE/a73_scripts.rs"
python3 "$HERE/a68_patch2.py" "$SCR"
echo "scratch ready: $SCR"
