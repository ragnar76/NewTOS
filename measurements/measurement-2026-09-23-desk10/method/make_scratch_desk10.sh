#!/usr/bin/env bash
# make_scratch_a109.sh <ws> <scratch-dir> <scripts.rs> [<before-dir>]
# The A108 scratch method (rsync of the crates -- never the shared tree --
# + a79_patch.py + the H02 injector with the given scripts + a78_patch2.py
# + key_step_patch.py + tree_dump_patch.py). With <before-dir> given, the five
# product files of A107/A109 are replaced by the copies found there (the
# "before" kernel = HEAD de487fe's files, extracted with `git show`, never a
# checkout).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; BEFORE="${4:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ -n "$BEFORE" ]; then
  for f in tos-api/src/aes/window.rs tos-api/src/aes/menu.rs tos-api/src/aes/mod.rs \
           kernel/src/desk/menu.rs kernel/src/desk/main.rs; do
    cp "$BEFORE/$f" "$SCR/$f"
  done
  echo "the five A107/A109 product files <- $BEFORE"
fi
python3 "/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/d7ff6c34-253d-4fd6-839f-efa08458ba32/scratchpad/desk10/method/a79_patch_desk10.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
python3 "$HERE/tree_dump_patch.py" "$SCR"
echo "scratch ready: $SCR"
