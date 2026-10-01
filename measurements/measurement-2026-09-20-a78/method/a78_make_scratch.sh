#!/usr/bin/env bash
# a78_make_scratch.sh <ws> <scratch-dir> <window.rs-source: tree|HEAD>
# Copies the crates into a scratch dir (never the shared working tree), applies
# the H02/A46 injector scaffolding + the H02 serial AES trace (scratch copy
# only), and optionally swaps in HEAD's window.rs ("before").
set -euo pipefail
WS="$1"; SCR="$2"; WIN="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ "$WIN" = "HEAD" ]; then
  git -C "$WS" show HEAD:tos-api/src/aes/window.rs > "$SCR/tos-api/src/aes/window.rs"
  echo "window.rs = HEAD ($(git -C "$WS" rev-parse --short HEAD))"
else
  echo "window.rs = working tree"
fi
python3 "$HERE/a78_patch.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$HERE/a78_scripts.rs"
python3 "$HERE/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
