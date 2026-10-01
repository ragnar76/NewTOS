#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <scripts.rs> [before]
#
# DESK19/DESK20 scratch method: rsync of the crates (never the shared tree)
# + a79_patch_desk10.py + the H02 injector with the given scripts
# + a78_patch2.py + key_step_patch.py + probe_patch.py + desk19_patch.py.
#
# With a fourth argument `before`, the five files DESK19/DESK20 touch are
# replaced by their committed HEAD versions (`git show HEAD:...`, read-only --
# the shared tree is never touched by git), which is the state before these two
# tickets. That is how the "before" pictures are made: same harness, same card,
# five files older.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; MODE="${4:-after}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ "$MODE" = "before" ]; then
  for f in kernel/src/desk/dir.rs kernel/src/desk/inf.rs kernel/src/desk/insapp.rs \
           kernel/src/desk/main.rs kernel/src/desk/global.rs kernel/src/desk/ins.rs; do
    (cd "$WS" && git show "HEAD:$f") > "$SCR/$f"
  done
  echo "desk/{dir,inf,insapp,main,global,ins}.rs reset to HEAD (before)"
fi
python3 "$HERE/a79_patch_desk10.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
python3 "$HERE/probe_patch.py" "$SCR"
python3 "$HERE/desk19_patch.py" "$SCR"
echo "scratch ready: $SCR ($MODE)"
