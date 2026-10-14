#!/usr/bin/env bash
#
# build.sh -- build a NewTOS native Rust program (DESK43 U6a, toolchain T1 of
# docs/analysis/desk43-u5-u10-review-2026-10-14.md §5).
#
# The Rust half replaces sdk/build.sh's step 2 (the C source); steps 1 and 3
# are sdk/build.sh's, unchanged in substance:
#   1. clang -c sdk/crt0.S                          -> crt0.o
#   2. cargo build -p <crate> --release (programs/) -> lib<crate>.a
#      target aarch64-unknown-none-softfloat, -C relocation-model=pic
#      (programs/.cargo/config.toml), panic=abort, fat LTO (programs/Cargo.toml)
#   3. rust-lld -T sdk/newtos.ld -pie --no-dynamic-linker -z text
#      crt0.o lib<crate>.a                          -> <OUT> (ELF ET_DYN)
#
# The output ELF is the program file itself (the loader parses ELF; no
# objcopy). crt0's `_start` calls the crate's `#[no_mangle] extern "C" fn
# main(basepage) -> i32` and Pterm's with its result.
#
# Usage:
#   programs/build.sh [crate] [output] [-- cargo args...]
#     crate    defaults to hello-gem
#     output   defaults to target/programs/HELLOGEM.NAP (repository root)
#     cargo args are passed to `cargo build`, e.g. `-- --features panic-test`
#
# PRGFLAGS: as sdk/build.sh -- the environment variable PRGFLAGS overrides
# crt0's note (default PF_STANDARD, 7).
set -euo pipefail

PROG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$PROG_DIR/.." && pwd)"
SDK_DIR="$WS/sdk"
CRATE="hello-gem"
OUT=""
if [ $# -gt 0 ] && [ "$1" != "--" ]; then CRATE="$1"; shift; fi
if [ $# -gt 0 ] && [ "$1" != "--" ]; then OUT="$1"; shift; fi
[ $# -gt 0 ] && [ "$1" = "--" ] && shift
OUT="${OUT:-$WS/target/programs/HELLOGEM.NAP}"
TARGET="aarch64-unknown-none-softfloat"
LIB="$WS/target/programs/$TARGET/release/lib${CRATE//-/_}.a"

CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
LLD="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/rust-lld"
[ -x "$LLD" ] || { echo "error: rust-lld not found at $LLD" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# 1. crt0, assembled exactly as sdk/build.sh does it.
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fno-stack-protector -fPIE -fno-builtin -Wall -Wextra "-I$SDK_DIR/include")
CRT0_DEFS=()
[ -n "${PRGFLAGS:-}" ] && CRT0_DEFS=("-DNEWTOS_PRGFLAGS=$PRGFLAGS")
echo "+ $CLANG ${CFLAGS[*]} ${CRT0_DEFS[*]:-} -c sdk/crt0.S"
"$CLANG" "${CFLAGS[@]}" ${CRT0_DEFS[@]+"${CRT0_DEFS[@]}"} -c "$SDK_DIR/crt0.S" -o "$WORK/crt0.o"

# 2. the Rust program as a static library.
echo "+ (cd programs && cargo build -p $CRATE --release $*)"
(cd "$PROG_DIR" && cargo build -p "$CRATE" --release "$@")

# 3. link, as sdk/build.sh step 3.
mkdir -p "$(dirname "$OUT")"
echo "+ rust-lld -flavor gnu -T sdk/newtos.ld -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none crt0.o $(basename "$LIB") -o $OUT"
"$LLD" -flavor gnu -T "$SDK_DIR/newtos.ld" -pie --no-dynamic-linker -z text \
    -z max-page-size=0x1000 --build-id=none \
    "$WORK/crt0.o" "$LIB" -o "$OUT"

echo "wrote $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
