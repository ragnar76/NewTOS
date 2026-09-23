#!/usr/bin/env bash
# make_scratch_desk18.sh <ws> <scratch-dir> <scripts.rs>
# DESK18 scratch method: rsync of the crates (never the shared tree)
# + a79_patch_desk10.py + the H02 injector with the given scripts
# + a78_patch2.py + key_step_patch.py + desk18_patch.py.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 "$HERE/a79_patch_desk10.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
python3 "$HERE/desk18_patch.py" "$SCR"
echo "scratch ready: $SCR"
