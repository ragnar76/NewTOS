#!/usr/bin/env bash
# make_scratch_a93.sh <ws> <scratch-dir> <scripts.rs> [<before-dir>]
# A93/A98: the A89 scratch method (rsync of the crates -- never the shared
# tree -- + a68_patch.py + a73_trace.py + the H02 injector with the given
# scripts + a68_patch2.py). With <before-dir> given, tos-api/src/aes/window.rs
# is replaced by the copy found there (the "before" kernel = HEAD 2ac0f9a's
# file, extracted with `git show`, never a checkout).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; BEFORE="${4:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ -n "$BEFORE" ]; then
  cp "$BEFORE/window.rs" "$SCR/tos-api/src/aes/window.rs"
  echo "window.rs <- $BEFORE"
fi
python3 "$HERE/a68_patch.py" "$SCR"
python3 "$HERE/a73_trace.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a68_patch2.py" "$SCR"
echo "scratch ready: $SCR"
