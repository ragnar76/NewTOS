#!/usr/bin/env bash
# make-inj-input01.sh <scratch root> <source tree> <prefix> [--prof] <actions...>
# INPUT01 injector kernels (adapted from measurement-2026-10-08-ofam-freeze/
# method/make-inj.sh): a scratch copy (rsync) of <source tree> +
# apply-injector-input01.py with <actions>.actions (+ desk-arm-m3.py for desk*
# and zv-start*), with --prof also apply-trapprof-input01.py, board-rpi4
# release, to <S>/<prefix>kinj-<actions>.img. <source tree> is the shared tree
# (INPUT01) or a `git archive` extract of ofam-2026 (the baseline); the shared
# tree is never modified and its target/ dir never used.
set -euo pipefail
S="${1:?scratch root}"; WS="${2:?source tree}"; P="${3:?prefix}"; shift 3
PROF=0; if [ "${1:-}" = --prof ]; then PROF=1; shift; fi
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
  cp "$WS/kernel/src/arch/aarch64/exceptions.rs" "$K/kernel/src/arch/aarch64/exceptions.rs"
  python3 "$HERE/apply-injector-input01.py" "$K" "$S/$x.rs"
  case "$x" in desk*|evb-desk*|zv-start*) python3 "$HERE/desk-arm-m3.py" "$K" > /dev/null ;; esac
  [ $PROF = 1 ] && python3 "$HERE/apply-trapprof-input01.py" "$K" > /dev/null
  ( cd "$K" && CARGO_TARGET_DIR="$S/target-inj" cargo build --package kernel --no-default-features \
      --features board-rpi4 --release ) > "$S/${P}kinj-$x.log" 2>&1 || { echo "BUILD FAILED $x"; tail -30 "$S/${P}kinj-$x.log"; exit 1; }
  "$OC" -O binary "$S/target-inj/aarch64-unknown-none-softfloat/release/kernel" "$S/${P}kinj-$x.img"
  shasum -a 256 "$S/${P}kinj-$x.img"
done
