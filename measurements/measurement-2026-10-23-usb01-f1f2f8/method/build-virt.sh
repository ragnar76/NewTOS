#!/bin/sh
# build-virt.sh <out.img> -- fresh qemu-virt dev kernel from the working tree,
# flat binary for QEMU -kernel (what `cargo xtask run` does). The rpi4 build
# shares target/.../debug/kernel, so the ELF is touched first to force the
# board's relink (docs: cargo-xtask board target footgun).
set -eu
ROOT=/Volumes/Daten/Development/PiRusTOS
OUT=$1
cd "$ROOT"
rm -f target/aarch64-unknown-none-softfloat/debug/kernel
cargo xtask build --board qemu-virt 2>&1 | tail -2
OBJCOPY=$(find "$(rustc --print sysroot)" -name rust-objcopy | head -1)
"$OBJCOPY" -O binary target/aarch64-unknown-none-softfloat/debug/kernel "$OUT"
shasum -a 256 "$OUT"
