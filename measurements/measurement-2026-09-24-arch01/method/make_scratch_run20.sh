#!/usr/bin/env bash
# make_scratch_run20.sh <ws> <scratch> <desk19-method-dir>
#
# ARCH01 acceptance tree: an rsync copy of the crates plus the DESK19/DESK20
# injector scaffolding, so the drag-launch gesture can be driven headlessly
# (memory `desktop-interactive-verification-real-hw`). Nothing but the copy is
# patched; the fix itself is in the shared tree.
#
# This is the command sequence that was run for
# measurement-2026-09-24-arch01, captured verbatim.
set -euo pipefail
WS="$1"; SCR="$2"; M="$3"
rm -rf "$SCR/tree"; mkdir -p "$SCR/tree" "$SCR/tmp/out"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/tree/"
done
python3 "$M/a79_patch_desk10.py" "$SCR/tree"
python3 "$M/apply_injector.py"   "$SCR/tree" "$M/d19-full.rs"
python3 "$M/a78_patch2.py"       "$SCR/tree"
python3 "$M/key_step_patch.py"   "$SCR/tree"
python3 "$M/probe_patch.py"      "$SCR/tree"
python3 "$M/desk19_patch.py"     "$SCR/tree"
( cd "$SCR/tree" && cargo run -p xtask -- build --board rpi4 )
bash "$M/seed.sh" "$SCR/tmp/base.img"
