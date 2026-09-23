#!/usr/bin/env bash
# make_scratch_desk06.sh <ws> <scratch-dir> <scripts.rs>
# DESK06 scratch method: rsync of the crates (never the shared tree)
# + a79_patch_desk10.py + the H02 injector with the given scripts
# + a78_patch2.py + key_step_patch.py + desk06_patch.py.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
D0809="$WS/measurement-2026-09-23-desk08-desk09/method"
D0102="$WS/measurement-2026-09-24-desk01-desk02/method"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 "$D0102/a79_patch_desk10.py" "$SCR"
python3 "$D0809/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$D0809/a78_patch2.py" "$SCR"
python3 "$D0809/key_step_patch.py" "$SCR"
python3 "$HERE/desk06_patch.py" "$SCR"
echo "scratch ready: $SCR"
