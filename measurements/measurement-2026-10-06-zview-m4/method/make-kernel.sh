#!/usr/bin/env bash
# make-kernel.sh <scratch root> -- HEAD's kernel (board-rpi4, release),
# unmodified, built into <S>/target (never the shared target/), -> <S>/kernel8.img
set -euo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
( cd "$WS" && CARGO_TARGET_DIR="$S/target" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/kbuild-rpi4.log" 2>&1
"$OC" -O binary "$S/target/aarch64-unknown-none-softfloat/release/kernel" "$S/kernel8.img"
shasum -a 256 "$S/kernel8.img"
