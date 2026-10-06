#!/usr/bin/env bash
# build-b2-kernel.sh <scratch root>
# Scratch rsync copy of the tree + the M3 IKBD injector (m3b.actions, input
# source only) + apply-b2-instr.py (serial prints only) -> <S>/kinj-b2.img.
# The shared tree and its target/ are not touched.
set -euo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
K="$S/kb2"; mkdir -p "$K"
for d in Cargo.toml Cargo.lock rust-toolchain.toml kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$K/"
done
python3 "$HERE/gen_script.py" "$HERE/m3b.actions" SCRIPT > "$S/m3b.rs"
python3 "$HERE/apply-injector-m3.py" "$K" "$S/m3b.rs" > /dev/null
python3 "$HERE/apply-b2-instr.py" "$K"
( cd "$K" && CARGO_TARGET_DIR="$S/target-b2" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/kb2-build.log" 2>&1 || { tail -40 "$S/kb2-build.log"; exit 1; }
"$OC" -O binary "$S/target-b2/aarch64-unknown-none-softfloat/release/kernel" "$S/kinj-b2.img"
( cd "$K" && git -C "$WS" rev-parse HEAD ) > "$S/kb2-base.txt"
shasum -a 256 "$S/kinj-b2.img"
