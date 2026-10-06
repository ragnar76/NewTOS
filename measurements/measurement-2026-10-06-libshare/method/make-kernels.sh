#!/usr/bin/env bash
# make-kernels.sh <scratch root>  (TASK/LDG-M3-B2, libshare run)
# Kernels for the runs, WITHOUT touching the shared tree or its target/ dir.
# No Rust source is changed by this task: every kernel is HEAD's.
#   <S>/kernel8-ls.img      HEAD's kernel (board-rpi4, release), unmodified
#   <S>/kinj-m3b.img        scratch copy + apply-injector-m3.py with m3b.actions
#   <S>/kinj-m3c.img        the same with m3c.actions (M3 acceptance script,
#                           written before the B1 fix: from step 3 on it no
#                           longer matches what the client does)
#   <S>/kinj-evb-acc.img    the same with evb-acc.actions (the acceptance
#                           script of measurement-2026-10-06-aes-evbutton =
#                           m3c with the post-B1 step 3, copied unchanged)
#   <S>/kinj-ls-instr.img   m3b injector + apply-ls-instr.py (serial prints of
#                           Malloc/Mxalloc/Mfree/Mshrink/Pexec/Pterm only)
# Copied from measurement-2026-10-06-ldg-m3/method/make-scratch-kernels.sh and
# ../ldg-b2/method/build-b2-kernel.sh; the injector only adds an input source.
set -euo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
mkdir -p "$S"
git -C "$WS" rev-parse HEAD > "$S/kernel-base.txt"
( cd "$WS" && CARGO_TARGET_DIR="$S/target" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/kbuild-rpi4.log" 2>&1
"$OC" -O binary "$S/target/aarch64-unknown-none-softfloat/release/kernel" "$S/kernel8-ls.img"
K="$S/kinj"; mkdir -p "$K"
for d in Cargo.toml Cargo.lock rust-toolchain.toml kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$K/"
done
build() { # build <actions> <out> [instr]
  python3 "$HERE/gen_script.py" "$HERE/$1.actions" SCRIPT > "$S/$1.rs"
  cp "$WS/kernel/src/host_env.rs" "$K/kernel/src/host_env.rs"
  cp "$WS/kernel/src/arch/aarch64/exceptions.rs" "$K/kernel/src/arch/aarch64/exceptions.rs"
  python3 "$HERE/apply-injector-m3.py" "$K" "$S/$1.rs" > /dev/null
  if [ "${3:-}" = instr ]; then python3 "$HERE/apply-ls-instr.py" "$K"; fi
  ( cd "$K" && CARGO_TARGET_DIR="$S/target-inj" cargo build --package kernel --no-default-features \
      --features board-rpi4 --release ) > "$S/kinj-$2.log" 2>&1 || { tail -30 "$S/kinj-$2.log"; exit 1; }
  "$OC" -O binary "$S/target-inj/aarch64-unknown-none-softfloat/release/kernel" "$S/$2"
}
build m3b kinj-m3b.img
build m3c kinj-m3c.img
build evb-acc kinj-evb-acc.img
build m3b kinj-ls-instr.img instr
cp "$WS/kernel/src/host_env.rs" "$K/kernel/src/host_env.rs"
cp "$WS/kernel/src/arch/aarch64/exceptions.rs" "$K/kernel/src/arch/aarch64/exceptions.rs"
shasum -a 256 "$S/kernel8-ls.img" "$S/kinj-m3b.img" "$S/kinj-m3c.img" "$S/kinj-evb-acc.img" "$S/kinj-ls-instr.img"
