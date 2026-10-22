#!/usr/bin/env bash
#
# build.sh -- compile + link a NewTOS Stage-1 native program (`.NCP`) with
# clang and LLVM lld (ld.lld, or rust-lld as fallback). No host libc, no CRT:
# freestanding AArch64.
#
# Pipeline:
#   1. clang -c crt0.S                -> crt0.o
#   2. clang -c examples/<name>.c     -> <name>.o     (with -Iinclude)
#   3. ld.lld (or rust-lld -flavor gnu) -T newtos.ld crt0.o <name>.o -> <OUT>
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
#
# PRGFLAGS: the program's PRGFLAGS long goes into crt0's ELF note (ARCH03 E8b,
# docs/api/gemdos/process/4b-pexec.md). Default PF_STANDARD (7); set the
# environment variable PRGFLAGS to override it for this build, e.g.
#   PRGFLAGS=0x2 sdk/build.sh prog.c PROG.NCP     (PF_TTRAMLOAD only)
set -euo pipefail

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="${1:-$SDK_DIR/examples/hello.c}"
OUT="${2:-/tmp/HELLO2.NCP}"

TARGET="aarch64-unknown-none-elf"
CFLAGS=(-target "$TARGET" -ffreestanding -nostdlib -O2
        -fno-stack-protector -fPIE -fno-builtin -Wall -Wextra
        "-I$SDK_DIR/include")

# --- locate clang ---
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }

# --- locate the LLVM ELF linker ---
# sdk/toolchain.sh (SDK01 I, K): NEWTOS_LLD, NEWTOS_LLVM_BIN, ld.lld on PATH,
# clang's directory, Homebrew's keg-only llvm/lld, /usr/lib/llvm-*/bin, and
# only then the pinned Rust nightly's rust-lld (asked from inside sdk/, so
# rust-toolchain.toml decides whatever the caller's cwd). newtos_lld adds
# -flavor gnu for rust-lld.
. "$SDK_DIR/toolchain.sh"
LLD="${LLD:-$(newtos_tool ld.lld)}" || exit 1

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "clang : $("$CLANG" --version | head -1)"
echo "lld   : $LLD"
echo

# 1. crt0 (+ the PRGFLAGS note value, if overridden)
CRT0_DEFS=()
[ -n "${PRGFLAGS:-}" ] && CRT0_DEFS=("-DNEWTOS_PRGFLAGS=$PRGFLAGS")
echo "+ $CLANG ${CFLAGS[*]} ${CRT0_DEFS[*]:-} -c $SDK_DIR/crt0.S -o crt0.o"
"$CLANG" "${CFLAGS[@]}" ${CRT0_DEFS[@]+"${CRT0_DEFS[@]}"} -c "$SDK_DIR/crt0.S" -o "$WORK/crt0.o"

# 2. C source
echo "+ $CLANG ${CFLAGS[*]} -c $SRC -o app.o"
"$CLANG" "${CFLAGS[@]}" -c "$SRC" -o "$WORK/app.o"

# 3. link (position-independent ET_DYN, linked at 0x1000 via newtos.ld; the
#    loader applies its R_AARCH64_RELATIVE relocations -- ARCH03 stage 0d)
LLD_FLAVOR="$(newtos_lld_flavor "$LLD")"
echo "+ $LLD ${LLD_FLAVOR:+$LLD_FLAVOR }-T $SDK_DIR/newtos.ld -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none crt0.o app.o -o $OUT"
newtos_lld -T "$SDK_DIR/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
     "$WORK/crt0.o" "$WORK/app.o" -o "$OUT"

echo
echo "wrote $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
