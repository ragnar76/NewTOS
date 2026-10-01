#!/usr/bin/env bash
# make_scratch_a48.sh <ws> <scratch-dir> <scripts.rs> [<before-dir>]
# A48 Tier 1: the A85/A99/A87 scratch method (rsync of the crates -- never the
# shared tree -- + a79_patch.py (H02 serial AES trace) + the H02 injector with
# the given scripts + a78_patch2.py (A46 dual arming)). With <before-dir>
# given, tos-api/src/aes/shell.rs and kernel/src/shell_loop.rs are replaced by
# the copies found there (the "before" kernel = HEAD bd9cd19's files,
# extracted with `git show`, never a checkout).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; BEFORE="${4:-}"
A79="$WS/measurement-2026-09-20-a79/method"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ -n "$BEFORE" ]; then
  cp "$BEFORE/shell.rs" "$SCR/tos-api/src/aes/shell.rs"
  cp "$BEFORE/shell_loop.rs" "$SCR/kernel/src/shell_loop.rs"
  echo "shell.rs + shell_loop.rs <- $BEFORE"
fi
python3 "$A79/a79_patch.py" "$SCR"
python3 "$A79/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$A79/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
