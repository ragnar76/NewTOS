#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <scripts.rs>
#
# PORT01 scratch method: rsync of the crates (never the shared tree) plus the
# standard interactive-verification patches -- a79_patch_desk10.py, the H02
# IKBD injector with the given script table, a78_patch2.py, key_step_patch.py
# and probe_patch.py (which prints the window items' click points, i.e. where
# the action lists' coordinates come from).
#
# PORT01 changes nothing in the kernel, so there is no "before" kernel here:
# the before/after pair is the two PROGRAMS on the card (method/build_before.sh
# and method/seed.sh), driven by the same kernel in the same run.
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
python3 "$HERE/probe_patch.py" "$SCR"
python3 "$HERE/port01_patch.py" "$SCR"
echo "scratch ready: $SCR"
