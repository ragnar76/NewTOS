#!/usr/bin/env bash
# missing_line_check.sh <ws> <scratch-dir>
# Simulates "a board feature without an About-box machine line": in a scratch
# copy, the rpi4 arm of desk::inf's MACHINE_NAME (and the compile_error's
# board list) is re-keyed to a feature that does not exist (`board-rpi4-x03`),
# then the kernel is checked for board-rpi4. Expected: the build fails with
# desk::inf's own compile_error, not with a silently wrong text.
set -uo pipefail
WS="$1"; SCR="$2"
mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --delete --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
F="$SCR/kernel/src/desk/inf.rs"
python3 - "$F" <<'PY'
import sys
p=sys.argv[1]; s=open(p).read()
a='#[cfg(feature = "board-rpi4")]\nconst MACHINE_NAME'
b='#[cfg(not(any(feature = "board-rpi4", feature = "board-qemu-virt")))]\ncompile_error!'
assert s.count(a)==1 and s.count(b)==1
s=s.replace(a,'#[cfg(feature = "board-rpi4-x03")]\nconst MACHINE_NAME')
s=s.replace(b,'#[cfg(not(any(feature = "board-rpi4-x03", feature = "board-qemu-virt")))]\ncompile_error!')
open(p,'w').write(s)
PY
cd "$SCR" && cargo check --package kernel --no-default-features --features board-rpi4 \
  --lib --target aarch64-unknown-none-softfloat 2>&1 | grep -E "^error" -A4
echo "cargo check exit: ${PIPESTATUS[0]}"
