#!/usr/bin/env bash
#
# build-ilp32.sh -- compile + link a NewTOS native program (`.NCP`) as an
# **ILP32** AArch64 ELF32 executable (ADR-020, the LP64->ILP32 app-ABI pivot).
#
# Rationale: unmodified Atari C assumes sizeof(pointer) == 4. The GNU
# bare-metal AArch64 toolchain's ILP32 mode (`-mabi=ilp32` + `-m aarch64elf32`)
# emits 4-byte pointers in a genuine ELF32 AArch64 image, so such code runs
# unmodified. The kernel stays LP64 internally; only the app is ILP32.
#
# This is the GNU sibling of `build.sh` (which uses clang + rust-lld to emit an
# LP64 ELF64 `.NCP`). It reuses the same crt0.S and newtos.ld unchanged:
#   - crt0.S reads p_hitpa from the basepage (a 32-bit TOS LONG at offset 4),
#     which is naturally correct on ILP32; and
#   - newtos.ld lays out one RWX PT_LOAD starting at 0x100. Under
#     `-m aarch64elf32` the linker emits an ELF32 header + ELF32 phdrs from the
#     same script (OUTPUT_FORMAT is selected by the emulation, not the script).
#
# Pipeline:
#   1. aarch64-elf-gcc -mabi=ilp32 -c crt0.S            -> crt0.o
#   2. aarch64-elf-gcc -mabi=ilp32 -c examples/<name>.c -> <name>.o  (-Iinclude)
#   3. aarch64-elf-ld  -m aarch64elf32 -T newtos.ld crt0.o <name>.o -> <OUT>
#
# The output ELF32 is the .NCP itself (the loader parses ELF directly; the
# ELF32-AArch64 parse path lives in tos-api/src/bdos/process/elf.rs).
#
# Confirm the result:
#   aarch64-elf-readelf -h <OUT>
#     -> Class ELF32, Machine AArch64, Type EXEC, Entry 0x100, one PT_LOAD.
#
# Usage:
#   sdk/build-ilp32.sh [source.c] [output.NCP]
#     source.c    defaults to sdk/examples/hello.c
#     output.NCP  defaults to /tmp/HELLO-ILP32.NCP
#   e.g. sdk/build-ilp32.sh sdk/examples/hello.c /tmp/HELLO-ILP32.NCP
#
# Toolchain override (defaults target Homebrew's aarch64-elf-* on macOS):
#   GCC=/path/to/aarch64-elf-gcc LD=/path/to/aarch64-elf-ld sdk/build-ilp32.sh
set -euo pipefail

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="${1:-$SDK_DIR/examples/hello.c}"
OUT="${2:-/tmp/HELLO-ILP32.NCP}"

# --- locate the GNU bare-metal AArch64 toolchain ---
# Prefer an explicit GCC/LD env override, else the Homebrew aarch64-elf-* names
# (via PATH), else the well-known Homebrew symlink location.
find_tool() {
    # $1 = env-var value (may be empty), $2 = tool basename
    local override="$1" name="$2"
    if [ -n "$override" ]; then echo "$override"; return 0; fi
    if command -v "$name" >/dev/null 2>&1; then echo "$name"; return 0; fi
    if [ -x "/opt/homebrew/bin/$name" ]; then echo "/opt/homebrew/bin/$name"; return 0; fi
    return 1
}
GCC="$(find_tool "${GCC:-}" aarch64-elf-gcc)" \
    || { echo "error: aarch64-elf-gcc not found (brew install aarch64-elf-gcc)" >&2; exit 1; }
LD="$(find_tool "${LD:-}" aarch64-elf-ld)" \
    || { echo "error: aarch64-elf-ld not found (brew install aarch64-elf-binutils)" >&2; exit 1; }

# ILP32: 4-byte pointers/longs. -mabi=ilp32 is deprecated-but-functional in
# recent GCC (still the only spelling); the warning is harmless. The rest match
# build.sh: freestanding, no CRT, no PIC, no builtins, warnings on.
CFLAGS=(-mabi=ilp32 -ffreestanding -nostdlib -O2
        -fno-stack-protector -fno-pic -fno-builtin -Wall -Wextra
        "-I$SDK_DIR/include")

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "gcc : $("$GCC" --version | head -1)"
echo "ld  : $("$LD" --version | head -1)"
echo

# 1. crt0
echo "+ $GCC ${CFLAGS[*]} -c $SDK_DIR/crt0.S -o crt0.o"
"$GCC" "${CFLAGS[@]}" -c "$SDK_DIR/crt0.S" -o "$WORK/crt0.o"

# 2. C source
echo "+ $GCC ${CFLAGS[*]} -c $SRC -o app.o"
"$GCC" "${CFLAGS[@]}" -c "$SRC" -o "$WORK/app.o"

# 3. link an ELF32 AArch64 static executable at the linked VAs (single PT_LOAD
#    via newtos.ld). `-m aarch64elf32` selects the ELF32 output format; the RWX
#    PT_LOAD is intentional (the loader maps every TPA page UserRwExec), so the
#    RWX warning is suppressed.
echo "+ $LD -m aarch64elf32 -T $SDK_DIR/newtos.ld --no-warn-rwx-segments crt0.o app.o -o $OUT"
"$LD" -m aarch64elf32 -T "$SDK_DIR/newtos.ld" --no-warn-rwx-segments \
      "$WORK/crt0.o" "$WORK/app.o" -o "$OUT"

echo
echo "wrote $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
