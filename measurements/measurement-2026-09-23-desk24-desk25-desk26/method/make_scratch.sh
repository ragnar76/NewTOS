#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <scripts.rs> [before]
#
# DESK24/DESK25/DESK26 scratch method: rsync of the crates (never the shared
# tree) + a79_patch_desk10.py + the H02 injector with the given scripts
# + a78_patch2.py + key_step_patch.py + probe_patch.py + desk24_patch.py.
#
# With a fourth argument `before`, kernel/src/desk/dir.rs is replaced by the
# committed HEAD version (`git show HEAD:...`, read-only -- the shared tree is
# never touched by git), which is the state before these three tickets. That is
# how the "before" pictures are made: same harness, same card, one file older.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; MODE="${4:-after}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ "$MODE" = "before" ]; then
  (cd "$WS" && git show HEAD:kernel/src/desk/dir.rs) > "$SCR/kernel/src/desk/dir.rs"
  echo "dir.rs reset to HEAD (before)"
fi
python3 "$HERE/a79_patch_desk10.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
python3 "$HERE/probe_patch.py" "$SCR"
python3 "$HERE/desk24_patch.py" "$SCR"
echo "scratch ready: $SCR ($MODE)"
