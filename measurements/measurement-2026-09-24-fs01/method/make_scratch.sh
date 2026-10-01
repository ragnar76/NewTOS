#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-tree> [unfix]
#
# FS01 scratch method: rsync of the crates (never the shared tree) +
# a79_patch_desk10.py (serial AES trace + the desktop-up flag the injector
# arms on) + apply_injector.py with method/empty-drive.rs + a78_patch2.py
# (script selection by arming source).
#
# With a third argument `unfix`, method/unfix.py edits the scratch copy of
# tos-api/src/bdos/fs/dir.rs back to the pre-FS01 behaviour -- the 8.3 entry
# is freed, the long-name slots in front of it are not. That is how the
# "before" image is made: same harness, same card, one function older. The
# shared tree is never touched, and no git command runs on it.
set -euo pipefail
WS="$1"; TREE="$2"; MODE="${3:-fixed}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$TREE"; mkdir -p "$TREE"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$TREE/"
done
python3 "$HERE/a79_patch_desk10.py" "$TREE"
python3 "$HERE/apply_injector.py"   "$TREE" "$HERE/empty-drive.rs"
python3 "$HERE/a78_patch2.py"       "$TREE"
if [ "$MODE" = "unfix" ]; then
  python3 "$HERE/unfix.py" "$TREE"
fi
echo "scratch ready: $TREE ($MODE)"
