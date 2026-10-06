#!/usr/bin/env bash
# make-kernels.sh <scratch root> -- kernels of TASK/LDG-M3-B1's runs, built
# WITHOUT touching the shared tree or its target/ dir (adapted from
# measurement-2026-10-06-ldg-m3/method/make-scratch-kernels.sh):
#   <S>/kernel8-evb.img     the shared tree's kernel (board-rpi4, release)
#   <S>/kinj-<x>.img        a scratch copy (rsync) + apply-injector-m3.py with
#                           <x>.actions, x in evb-acc evb-tera evb-qed (evb-desk3a on request); for
#                           evb-desk* also desk-arm-m3.py (arms at desktop up)
# The kernel code under test is the shared tree's; the injector only adds an
# input source. SRC=<tree> builds another source tree instead (the baseline:
# `git archive HEAD` extracted into the scratch dir), PFX=<prefix> names its
# images <prefix>kinj-<x>.img; ONLY="<x> ..." limits the injector kernels.
set -euo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="${SRC:-$(cd "$HERE/../../.." && pwd)}"; P="${PFX:-}"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
mkdir -p "$S"
if [ -z "$P" ]; then
( cd "$WS" && CARGO_TARGET_DIR="$S/target" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/kbuild-rpi4.log" 2>&1
"$OC" -O binary "$S/target/aarch64-unknown-none-softfloat/release/kernel" "$S/kernel8-evb.img"
fi
K="$S/${P}kinj"; mkdir -p "$K"
for d in Cargo.toml Cargo.lock rust-toolchain.toml kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$K/"
done
build() { # build <actions> <out> [desk]
  python3 "$HERE/gen_script.py" "$HERE/$1.actions" SCRIPT > "$S/$1.rs"
  cp "$WS/kernel/src/host_env.rs" "$K/kernel/src/host_env.rs"
  cp "$WS/kernel/src/desk/mod.rs" "$K/kernel/src/desk/mod.rs"
  python3 "$HERE/apply-injector-m3.py" "$K" "$S/$1.rs" > /dev/null
  [ "${3:-}" = desk ] && python3 "$HERE/desk-arm-m3.py" "$K" > /dev/null
  ( cd "$K" && CARGO_TARGET_DIR="$S/${P}target-inj" cargo build --package kernel --no-default-features \
      --features board-rpi4 --release ) > "$S/${P}kinj-$1.log" 2>&1
  "$OC" -O binary "$S/${P}target-inj/aarch64-unknown-none-softfloat/release/kernel" "$S/$2"
}
for x in ${ONLY:-evb-acc evb-tera evb-qed evb-desk}; do
  if [ "${x#evb-desk}" != "$x" ]; then build $x ${P}kinj-$x.img desk; else build $x ${P}kinj-$x.img; fi
done
cp "$WS/kernel/src/host_env.rs" "$K/kernel/src/host_env.rs"
cp "$WS/kernel/src/desk/mod.rs" "$K/kernel/src/desk/mod.rs"
shasum -a 256 "$S"/kernel8-evb.img "$S"/*kinj-*.img
