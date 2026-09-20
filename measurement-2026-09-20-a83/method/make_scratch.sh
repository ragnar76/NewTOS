#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <scripts.rs>
# A83 (2026-09-20): the A79 final method — copies the crates into a scratch
# dir (never the shared working tree) and applies only the H02/A46/A78
# injector scaffolding + the H02 serial AES trace (scratch copy only), with
# the given injector script file (final_scripts.rs = the A79 desk/tera
# scripts, nav_scripts.rs = the folder-open + closer navigate script).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 "$HERE/a79_patch.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
