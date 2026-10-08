#!/usr/bin/env bash
# make-kernels-keyowner.sh <scratch root> <head|new> -- the KEYOWNER kernels,
# built WITHOUT touching the shared tree or its target/ dir:
#   head: source = `git archive HEAD` (e3400de9) extracted to <S>/head-src
#   new : source = the shared tree (HEAD + this change, uncommitted)
#   <S>/<P>-kernel8-evb.img   plain board-rpi4 release kernel (APITEST runs)
#   <S>/<P>-kinj-<x>.img      injector kernels (make-inj-input01.sh, INPUT01's
#                             scripted IKBD injector, no profiler)
# Separate cargo target dirs per kernel set (<S>/target-plain-<P>,
# <S>/target-inj-<P>): a shared one let the "new" build pick up the tos-api
# compiled from the HEAD extract (first attempt: new-plain failed to compile).
set -euo pipefail
S="${1:?scratch root}"; P="${2:?head|new}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
if [ "$P" = head ]; then
  SRC="$S/head-src"; rm -rf "$SRC"; mkdir -p "$SRC"
  git -C "$WS" archive HEAD Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools \
    | tar -x -C "$SRC"
  git -C "$WS" rev-parse HEAD > "$S/head-src.rev"
else
  SRC="$WS"
fi
( cd "$SRC" && CARGO_TARGET_DIR="$S/target-plain-$P" cargo build --package kernel --no-default-features \
    --features board-rpi4 --release ) > "$S/$P-plain.log" 2>&1
"$OC" -O binary "$S/target-plain-$P/aarch64-unknown-none-softfloat/release/kernel" "$S/$P-kernel8-evb.img"
shasum -a 256 "$S/$P-kernel8-evb.img"
bash "$HERE/make-inj-input01.sh" "$S" "$SRC" "$P-" \
  evb-acc evb-desk evb-tera evb-qed evb-desk3a zv-dclick zv-browser zv-quit doom \
  deskw qedw qedo teraw zvfull desk-sikeys desk-si si-tera si-qed desk-emucon | grep img
