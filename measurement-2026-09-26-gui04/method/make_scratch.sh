#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <scripts.rs>
#
# rsync of the crates into the job scratch (NEVER the shared tree -- the
# injector is measurement scaffolding and must not end up in the product) plus
# the GUI04 IKBD/USB injector with the given script table.
#
# The injector is the only patch: unlike the DESK series this measurement needs
# no geometry probes, because run A reports over the serial console and run B
# only has to show a window scrolling.
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPTS="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPTS"
echo "scratch ready: $SCR"
