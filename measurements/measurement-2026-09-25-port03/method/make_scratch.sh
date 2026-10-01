#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <scripts.rs>
#
# PORT03 scratch method, identical in shape to PORT01's: an rsync of the crates
# into the job scratch (NEVER the shared tree) plus the standard
# interactive-verification patches -- a79_patch_desk10.py, the H02 IKBD
# injector with the given script table, a78_patch2.py, key_step_patch.py,
# probe_patch.py (which prints the window items' click points, i.e. where the
# action lists' coordinates come from) and t10_workaround.py (TASK/T10: the
# dual arming would otherwise pick the empty SCRIPT_TERA).
#
# PORT03 changes NO kernel code: the swap is in sdk/cflib. The kernel in the
# scratch tree is the shared tree's kernel, patched only with the measurement
# scaffolding; the thing under test is the PROGRAM on the card.
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
python3 "$HERE/t10_workaround.py" "$SCR"
echo "scratch ready: $SCR"
