#!/usr/bin/env bash
# make_scratch_d50.sh <ws> <scratch-dir> <scripts.rs>
# DESK50 scratch method, as DESK46/47: rsync of the crates (never the shared
# tree) + the GUI04 IKBD injector with the given script table + the GUI04
# desktop-arming patch + desk50_patch.py.  `target/` is kept between runs.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/desk_arm_patch.py" "$SCR"
python3 "$HERE/desk50_patch.py" "$SCR"
echo "scratch ready: $SCR"
