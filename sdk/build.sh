#!/usr/bin/env bash
#
# build.sh -- compile + link a NewTOS Stage-1 native program (`.NCP`) with
# clang and LLVM lld (rust-lld). No host libc, no CRT: freestanding AArch64.
#
# Pipeline:
#   1. clang -c crt0.S                -> crt0.o
#   2. clang -c examples/<name>.c     -> <name>.o     (with -Iinclude)
#   3. rust-lld -flavor gnu -T newtos.ld crt0.o <name>.o -> <OUT>
#
# The output ELF is the .NCP itself (the loader parses ELF directly; no
# flat-binary objcopy step, unlike the kernel image).
#
# The linked ELF carries a text (R-X) and, when the program has .data/.bss, a
# data (RW-) PT_LOAD, each of which may span many pages; the multi-page-TPA
# loader copies them, zeroes the BSS tail, fills the basepage segment fields
# from them, and maps the whole per-process TPA. No image-size cap beyond
# `TPA_SIZE - stack reserve`.
#
# Usage:
#   sdk/build.sh [source.c] [output.NCP]
#     source.c    defaults to sdk/examples/hello.c
#     output.NCP  defaults to /tmp/HELLO2.NCP
#   e.g. sdk/build.sh sdk/examples/big.c /tmp/BIG.NCP
set -euo pipefail

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="${1:-$SDK_DIR/examples/hello.c}"
OUT="${2:-/tmp/HELLO2.NCP}"

TARGET="aarch64-unknown-none-elf"
CFLAGS=(-target "$TARGET" -ffreestanding -nostdlib -O2
        -fno-stack-protector -fno-pic -fno-builtin -Wall -Wextra
        "-I$SDK_DIR/include")

# --- locate clang ---
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }

# --- locate an LLVM ELF linker (rust-lld ships with the Rust toolchain) ---
find_lld() {
    if command -v ld.lld >/dev/null 2>&1; then echo "ld.lld"; return; fi
    if command -v rust-lld >/dev/null 2>&1; then echo "rust-lld -flavor gnu"; return; fi
    local sysroot; sysroot="$(rustc --print sysroot 2>/dev/null || true)"
    if [ -n "$sysroot" ]; then
        local cand; cand="$(find "$sysroot"/lib/rustlib -name rust-lld 2>/dev/null | head -1)"
        [ -n "$cand" ] && { echo "$cand -flavor gnu"; return; }
    fi
    [ -x "$HOME/.cargo/bin/rust-lld" ] && { echo "$HOME/.cargo/bin/rust-lld -flavor gnu"; return; }
    return 1
}
LLD="$(find_lld)" || { echo "error: no lld/rust-lld found" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "clang : $("$CLANG" --version | head -1)"
echo "lld   : $LLD"
echo

# 1. crt0
echo "+ $CLANG ${CFLAGS[*]} -c $SDK_DIR/crt0.S -o crt0.o"
"$CLANG" "${CFLAGS[@]}" -c "$SDK_DIR/crt0.S" -o "$WORK/crt0.o"

# 2. C source
echo "+ $CLANG ${CFLAGS[*]} -c $SRC -o app.o"
"$CLANG" "${CFLAGS[@]}" -c "$SRC" -o "$WORK/app.o"

# 3. link (no-pie static ELF at the linked VAs; single PT_LOAD via newtos.ld)
echo "+ $LLD -T $SDK_DIR/newtos.ld -no-pie -z max-page-size=0x1000 --build-id=none crt0.o app.o -o $OUT"
# shellcheck disable=SC2086
$LLD -T "$SDK_DIR/newtos.ld" -no-pie -z max-page-size=0x1000 --build-id=none \
     "$WORK/crt0.o" "$WORK/app.o" -o "$OUT"

echo
echo "wrote $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
