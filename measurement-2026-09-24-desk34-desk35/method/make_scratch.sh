#!/usr/bin/env bash
# make_scratch.sh <scratch-tree> <scripts.rs> [before]
#
# DESK34/DESK35 scratch method: rsync of the crates (never the shared tree) +
# the standard injector scaffolding + probe_patch.py.
#
# With a third argument `before`, the two files this batch touches are replaced
# by their committed HEAD versions (`git show HEAD:...`, read-only -- the shared
# tree is never touched by git), which is the state before DESK34 and DESK35.
# That is how the "before" pictures are made: same harness, same card, two files
# older.
set -euo pipefail
TREE="$1"; SCRIPTS="$2"; MODE="${3:-after}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS=/Volumes/Daten/Development/PiRusTOS
rm -rf "$TREE"; mkdir -p "$TREE"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk xtask; do
  [ -e "$WS/$d" ] && rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$TREE/"
done
if [ "$MODE" = "before" ]; then
  for f in kernel/src/desk/main.rs kernel/src/shell_loop.rs; do
    (cd "$WS" && git show "HEAD:$f") > "$TREE/$f"
  done
  echo "desk/main.rs, shell_loop.rs reset to HEAD (before)"
fi
python3 "$HERE/a79_patch_desk10.py" "$TREE"
python3 "$HERE/apply_injector.py"   "$TREE" "$SCRIPTS"
python3 "$HERE/a78_patch2.py"       "$TREE"
python3 "$HERE/key_step_patch.py"   "$TREE"
python3 "$HERE/probe_patch.py"      "$TREE"
echo "scratch ready: $TREE ($MODE)"
