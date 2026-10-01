#!/usr/bin/env bash
# make_scratch_final.sh <ws> <scratch-dir>
# A79 final (after the look decision, 2026-09-20): copies the crates into a
# scratch dir (never the shared working tree) and applies only the
# H02/A46/A78 injector scaffolding + the H02 serial AES trace (scratch copy
# only). There is no look switch any more — the tree's draw_frame renders
# the literal colour words, and that is what this kernel shows.
set -euo pipefail
WS="$1"; SCR="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if grep -q "FRAME_LOOK" "$SCR/tos-api/src/aes/window.rs"; then
  echo "FRAME_LOOK still present -- this script is for the tree after the switch removal" >&2; exit 1
fi
python3 "$HERE/a79_patch.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$HERE/final_scripts.rs"
python3 "$HERE/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
