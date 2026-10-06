#!/usr/bin/env bash
# make-scratch-kernels.sh <scratch root>
# Builds the kernels of TASK/LDG-M3's runs WITHOUT touching the shared tree or
# its target/ dir:
#   <S>/kernel8-m3.img        HEAD's kernel (board-rpi4, release), unmodified
#   <S>/kinj-<x>.img          a scratch copy (rsync) + apply-injector-m3.py with
#                             method/<x>.actions, for x in m3b m3c
#   <S>/kinj-m3a-trace.img    the same for m3a.actions, plus --features aes-trace
#                             (writes C:\AESTRACE.BIN at the client's Pterm)
# The kernel code under test is the shared tree's; the injector only adds an
# input source (see apply-injector-m3.py).
set -euo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
mkdir -p "$S"
( cd "$WS" && CARGO_TARGET_DIR="$S/target" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/kbuild-rpi4.log" 2>&1
"$OC" -O binary "$S/target/aarch64-unknown-none-softfloat/release/kernel" "$S/kernel8-m3.img"
K="$S/kinj"; mkdir -p "$K"
for d in Cargo.toml Cargo.lock rust-toolchain.toml kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$K/"
done
build() { # build <actions> <features> <out>
  python3 "$HERE/gen_script.py" "$HERE/$1.actions" SCRIPT > "$S/$1.rs"
  cp "$WS/kernel/src/host_env.rs" "$K/kernel/src/host_env.rs"
  python3 "$HERE/apply-injector-m3.py" "$K" "$S/$1.rs" > /dev/null
  ( cd "$K" && CARGO_TARGET_DIR="$S/target-inj" cargo build --package kernel --no-default-features \
      --features "$2" --release ) > "$S/kinj-$1.log" 2>&1
  "$OC" -O binary "$S/target-inj/aarch64-unknown-none-softfloat/release/kernel" "$S/$3"
}
build m3b board-rpi4 kinj-m3b.img
build m3c board-rpi4 kinj-m3c.img
build m3a board-rpi4,aes-trace kinj-m3a-trace.img
cp "$WS/kernel/src/host_env.rs" "$K/kernel/src/host_env.rs"
shasum -a 256 "$S/kernel8-m3.img" "$S/kinj-m3b.img" "$S/kinj-m3c.img" "$S/kinj-m3a-trace.img"
