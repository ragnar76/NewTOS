#!/usr/bin/env bash
# make-probe-kernel.sh <workspace> <scratch> -- fresh scratch copy of the tree,
# apply the FS08 probe, build board-rpi4 release into <scratch>/target,
# write <scratch>/bin/kernel8-fs08probe.img and print its sha256.
set -euo pipefail
WS="$1"; S="$2"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$S/tree"; mkdir -p "$S/tree" "$S/bin"
( cd "$WS" && rsync -a --exclude target .cargo Cargo.toml Cargo.lock rust-toolchain.toml kernel tos-api tos-types tools "$S/tree/" )
git -C "$WS" rev-parse HEAD > "$S/tree/BASE_HEAD"
python3 "$HERE/fs08_probe_patch.py" "$S/tree"
( cd "$S/tree" && CARGO_TARGET_DIR="$S/target" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/kbuild-probe.log" 2>&1 || { grep -A8 "^error" "$S/kbuild-probe.log"; exit 1; }
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
"$OC" -O binary "$S/target/aarch64-unknown-none-softfloat/release/kernel" "$S/bin/kernel8-fs08probe.img"
shasum -a 256 "$S/bin/kernel8-fs08probe.img"
