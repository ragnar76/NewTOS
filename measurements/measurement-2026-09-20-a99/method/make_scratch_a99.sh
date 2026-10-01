#!/usr/bin/env bash
# make_scratch_a99.sh <ws> <scratch-dir> <scripts.rs> [<before-dir>]
# A99: the A85/A79 scratch method (rsync of the crates -- never the shared
# tree -- + a79_patch.py + the H02 injector with the given A99 scripts +
# a78_patch2.py). With <before-dir> given, tos-api/src/aes/object.rs and
# tos-api/src/aes/menu.rs are replaced by the copies found there (the "before"
# kernel = HEAD 30a7156's files, extracted with `git show`, never a checkout).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; BEFORE="${4:-}"
A79="$WS/measurement-2026-09-20-a79/method"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ -n "$BEFORE" ]; then
  cp "$BEFORE/object.rs" "$SCR/tos-api/src/aes/object.rs"
  cp "$BEFORE/menu.rs" "$SCR/tos-api/src/aes/menu.rs"
  echo "object.rs + menu.rs <- $BEFORE"
fi
python3 "$A79/a79_patch.py" "$SCR"
python3 "$A79/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$A79/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
