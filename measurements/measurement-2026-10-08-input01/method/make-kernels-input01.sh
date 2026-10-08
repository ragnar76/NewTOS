#!/usr/bin/env bash
# make-kernels-input01.sh <scratch root> -- the regression kernels of INPUT01,
# built from the shared tree WITHOUT touching it or its target/ dir:
#   <S>/new-kernel8-evb.img   the tree's kernel (board-rpi4, release) -- api runs
#   <S>/new-kinj-<x>.img      injector kernels (make-inj-input01.sh, no profiler)
set -euo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
( cd "$WS" && CARGO_TARGET_DIR="$S/target-chk" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/kbuild-rpi4.log" 2>&1
"$OC" -O binary "$S/target-chk/aarch64-unknown-none-softfloat/release/kernel" "$S/new-kernel8-evb.img"
shasum -a 256 "$S/new-kernel8-evb.img"
bash "$HERE/make-inj-input01.sh" "$S" "$WS" new- evb-acc evb-desk evb-tera evb-qed evb-desk3a \
  zv-dclick zv-browser zv-quit doom | grep img
