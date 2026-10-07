#!/usr/bin/env bash
# make-inj.sh <scratch root> <source tree> <prefix> <actions...> -- injector
# kernels of TASK/ACCTITLE (adapted from measurement-2026-10-07-k1/method/
# make-kernels.sh): a scratch copy (rsync) of <source tree> + apply-injector-m3.py
# with <actions>.actions (+ desk-arm-m3.py for desk*), board-rpi4 release, to
# <S>/<prefix>kinj-<actions>.img. <source tree> is the shared tree (the fix)
# or a `git archive HEAD` extract (the baseline); the shared tree is never
# modified, its target/ dir never used.
set -euo pipefail
S="${1:?scratch root}"; WS="${2:?source tree}"; P="${3:?prefix}"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
K="$S/${P}kinj"; mkdir -p "$K"
for d in Cargo.toml Cargo.lock rust-toolchain.toml kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$K/"
done
for x in "$@"; do
  python3 "$HERE/gen_script.py" "$HERE/$x.actions" SCRIPT > "$S/$x.rs"
  cp "$WS/kernel/src/host_env.rs" "$K/kernel/src/host_env.rs"
  cp "$WS/kernel/src/desk/mod.rs" "$K/kernel/src/desk/mod.rs"
  python3 "$HERE/apply-injector-m3.py" "$K" "$S/$x.rs" > /dev/null
  if [ "${x#desk}" != "$x" ]; then python3 "$HERE/desk-arm-m3.py" "$K" > /dev/null; fi
  ( cd "$K" && CARGO_TARGET_DIR="$S/${P}target-inj" cargo build --package kernel --no-default-features \
      --features board-rpi4 --release ) > "$S/${P}kinj-$x.log" 2>&1
  "$OC" -O binary "$S/${P}target-inj/aarch64-unknown-none-softfloat/release/kernel" "$S/${P}kinj-$x.img"
  shasum -a 256 "$S/${P}kinj-$x.img"
done
