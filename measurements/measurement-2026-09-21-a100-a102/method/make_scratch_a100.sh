#!/usr/bin/env bash
# make_scratch_a100.sh <ws> <scratch-dir> <scripts.rs> [<before-dir>]
# A100/A102: the A87 scratch method (rsync of the crates -- never the shared
# tree -- + a79_patch.py + the H02 injector with the given scripts + a78_patch2.py
# + key_step_patch.py) plus tree_dump_patch.py (serial dump of the desktop's
# registered menu geometry). With <before-dir> given, tos-api/src/aes/
# {input,event,menu,window}.rs and kernel/src/desk/menu.rs are replaced by the
# copies found there (the "before" kernel = HEAD 022d6d0's files, extracted
# with `git show`, never a checkout).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"; BEFORE="${4:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
A79="$WS/measurement-2026-09-20-a79/method"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ -n "$BEFORE" ]; then
  for f in input.rs menu.rs window.rs; do cp "$BEFORE/$f" "$SCR/tos-api/src/aes/$f"; done
  cp "$BEFORE/desk-menu.rs" "$SCR/kernel/src/desk/menu.rs"
  echo "tos-api aes/{input,menu,window}.rs + kernel desk/menu.rs <- $BEFORE"
fi
python3 "$A79/a79_patch.py" "$SCR"
python3 "$A79/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$A79/a78_patch2.py" "$SCR"
python3 "$HERE/key_step_patch.py" "$SCR"
python3 "$HERE/tree_dump_patch.py" "$SCR"
echo "scratch ready: $SCR"
