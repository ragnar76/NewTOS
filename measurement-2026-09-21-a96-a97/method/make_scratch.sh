#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <scripts.rs> [<before-dir>]
# A96/A97: the A87 scratch method (rsync of the crates -- never the shared
# tree -- + a79_patch.py + the H02 injector with the given scripts + a78_patch2.py
# + key_step_patch.py). With <before-dir> given, the product files of this
# ticket (tos-api/src/aes/{object,window}.rs, kernel/src/desk/main.rs) are
# replaced by the copies found there (the "before" kernel = HEAD 1c4414a's
# files, extracted with `git show`, never a checkout).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; BEFORE="${4:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ -n "$BEFORE" ]; then
  cp "$BEFORE/object.rs" "$SCR/tos-api/src/aes/object.rs"
  cp "$BEFORE/window.rs" "$SCR/tos-api/src/aes/window.rs"
  cp "$BEFORE/main.rs" "$SCR/kernel/src/desk/main.rs"
  echo "object.rs + window.rs + desk/main.rs <- $BEFORE"
fi
python3 "$HERE/a79_patch.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
echo "scratch ready: $SCR"
