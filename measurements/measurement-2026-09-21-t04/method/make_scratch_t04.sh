#!/usr/bin/env bash
# make_scratch_t04.sh <ws> <scratch-dir> <scripts.rs>
# T04: the A79/A100 scratch method (rsync of the crates -- never the shared
# tree -- + a79_patch.py + the H02 injector with the given scripts +
# a78_patch2.py). No product-file replacement: T04 changes no kernel product
# code (a doc comment in tos-api/src/aes/mod.rs only); BEFORE and AFTER share
# this one scratch kernel and differ in the DESKTOP.NAP on the SD image.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 "$HERE/a79_patch.py" "$SCR"
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
python3 "$HERE/a78_patch2.py" "$SCR"
echo "scratch ready: $SCR"
