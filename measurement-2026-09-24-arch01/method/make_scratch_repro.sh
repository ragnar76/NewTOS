#!/usr/bin/env bash
# make_scratch_repro.sh <ws> <scratch>
#
# ARCH01, "forced race" tree: an rsync copy of the crates (never the shared
# tree) plus force_race_patch.py. Build it, boot it on the standard card and
# the C:\AUTO\HELLO.NCP launch hits the window on every run.
#
# This is the command sequence that was run for
# measurement-2026-09-24-arch01, captured verbatim.
set -euo pipefail
WS="$1"; SCR="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR/tree"; mkdir -p "$SCR/tree" "$SCR/tmp"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools sdk; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/tree/"
done
python3 "$HERE/force_race_patch.py" "$SCR/tree"
( cd "$SCR/tree" && cargo run -p xtask -- build --board rpi4 )
bash "$SCR/tree/tools/make-sd-image.sh" "$SCR/tmp/base.img" 64
cp "$SCR/tmp/base.img" "$SCR/tmp/run.img"
bash "$HERE/runboot.sh" "$SCR/tree/kernel8.img" "$SCR/tmp/run.img" "$SCR/tmp/forced-before.log" 180
echo "--- the 'after' half: the fixed user_entry.rs + the same delay ---"
cp "$WS/kernel/src/arch/aarch64/user_entry.rs" "$SCR/tree/kernel/src/arch/aarch64/user_entry.rs"
python3 "$HERE/force_race_patch.py" "$SCR/tree"
( cd "$SCR/tree" && cargo run -p xtask -- build --board rpi4 )
cp "$SCR/tmp/base.img" "$SCR/tmp/run2.img"
bash "$HERE/runboot.sh" "$SCR/tree/kernel8.img" "$SCR/tmp/run2.img" "$SCR/tmp/forced-after.log" 240
