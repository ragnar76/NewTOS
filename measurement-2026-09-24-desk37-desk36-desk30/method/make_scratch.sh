#!/usr/bin/env bash
# make_scratch.sh <scratch-tree> <scripts.rs> [before|after] [probe]
#
# A fourth argument `probe` adds probe_patch.py (the serial icon/menu dump).
# It is left OFF for the DESK36 runs: its hook sits in do_wredraw, which the new
# drag loop calls once per highlight change.
#
# DESK37/DESK36 scratch method: rsync of the crates (never the shared tree) +
# the standard injector scaffolding + probe_patch.py.
#
# With a third argument `before`, every file this batch touches is replaced by
# its committed HEAD version (`git show HEAD:...`, read-only -- the shared tree
# is never touched by git) and the one NEW file is removed, which is the state
# before DESK37 and DESK36. That is how the "before" pictures are made: same
# harness, same card, the product files older.
set -euo pipefail
TREE="$1"; SCRIPTS="$2"; MODE="${3:-after}"; PROBE="${4:-noprobe}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS=/Volumes/Daten/Development/PiRusTOS
rm -rf "$TREE"; mkdir -p "$TREE"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk xtask; do
  [ -e "$WS/$d" ] && rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$TREE/"
done
if [ "$MODE" = "before" ]; then
  for f in kernel/src/shell_loop.rs \
           kernel/src/desk/main.rs kernel/src/desk/mod.rs kernel/src/desk/act.rs \
           kernel/src/desk/app.rs kernel/src/desk/dir.rs kernel/src/desk/global.rs \
           kernel/src/desk/inf.rs kernel/src/desk/ins.rs; do
    (cd "$WS" && git show "HEAD:$f") > "$TREE/$f"
  done
  rm -f "$TREE/kernel/src/desk/vdibind.rs"
  echo "9 files reset to HEAD, vdibind.rs removed (before)"
fi
python3 "$HERE/a79_patch_desk10.py" "$TREE"
python3 "$HERE/apply_injector.py"   "$TREE" "$SCRIPTS"
python3 "$HERE/a78_patch2.py"       "$TREE"
python3 "$HERE/key_step_patch.py"   "$TREE"
if [ "$PROBE" = "probe" ]; then python3 "$HERE/probe_patch.py" "$TREE"; fi
echo "scratch ready: $TREE ($MODE)"
