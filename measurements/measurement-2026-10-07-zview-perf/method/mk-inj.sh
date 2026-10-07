#!/usr/bin/env bash
# mk-inj.sh <S> <actions-name> -- injector kernel kinj-<name>.img from the
# ZVPROF scratch tree <S>/tree (copied to <S>/kinj, + apply-injector-m3.py).
set -euo pipefail
S="$1"; N="$2"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OC="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-objcopy"
python3 "$HERE/gen_script.py" "$HERE/$N.actions" SCRIPT > "$S/$N.rs"
rsync -a --delete --exclude target "$S/tree/" "$S/kinj/"
python3 "$HERE/apply-injector-m3.py" "$S/kinj" "$S/$N.rs" > /dev/null
( cd "$S/kinj" && CARGO_TARGET_DIR="$S/target-inj" cargo build --package kernel --no-default-features --features board-rpi4 --release ) > "$S/kinj-$N.log" 2>&1
"$OC" -O binary "$S/target-inj/aarch64-unknown-none-softfloat/release/kernel" "$S/kinj-$N.img"
shasum -a 256 "$S/kinj-$N.img"
