#!/usr/bin/env bash
# make_scratch_before.sh <ws> <scratch-dir> <scripts.rs>
#
# The DESK32 "before" tree: exactly make_scratch.sh, except that
# kernel/src/desk/menu.rs is taken from the COMMITTED state (HEAD 12fddc1)
# instead of from the working tree, i.e. with the padding still inside the
# MENUS strings.  That file is the whole of DESK32's change, so this is the
# before/after pair for the measured drop-down widths.
#
# `git show HEAD:<path>` is read-only; the shared tree is never touched (no
# checkout/restore/clean/stash/reset).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
( cd "$WS" && git show HEAD:kernel/src/desk/menu.rs ) > "$SCR/kernel/src/desk/menu.rs"
echo "menu.rs reverted to HEAD (DESK32 'before')"
python3 "$HERE/a79_patch_desk10.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
python3 "$HERE/probe_patch.py" "$SCR"
python3 "$HERE/t10_workaround.py" "$SCR"
echo "scratch ready: $SCR"
