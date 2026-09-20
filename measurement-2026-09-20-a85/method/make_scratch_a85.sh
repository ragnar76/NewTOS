#!/usr/bin/env bash
# make_scratch_a85.sh <ws> <scratch-dir> [<object.rs to substitute>]
# A85: the A79 final scratch method (rsync of the crates -- never the shared
# tree -- + a79_patch.py + the H02 injector with the A85 scripts + a78_patch2),
# optionally with tos-api/src/aes/object.rs replaced by a given copy (the
# "before" kernel = HEAD's object.rs).
set -euo pipefail
WS="$1"; SCR="$2"; OBJ="${3:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
A79="$WS/measurement-2026-09-20-a79/method"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ -n "$OBJ" ]; then cp "$OBJ" "$SCR/tos-api/src/aes/object.rs"; echo "object.rs <- $OBJ"; fi
python3 "$A79/a79_patch.py" "$SCR"
python3 "$A79/apply_injector.py" "$SCR" "$HERE/${A85_SCRIPTS:-a85_scripts.rs}"
python3 "$A79/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
