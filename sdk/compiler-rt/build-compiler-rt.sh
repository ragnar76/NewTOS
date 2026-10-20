#!/usr/bin/env bash
#
# build-compiler-rt.sh -- build LLVM compiler-rt's "builtins" (the compiler
# runtime clang emits calls to) into sdk/compiler-rt/libclang_rt.builtins.a for
# NewTOS/AArch64 (TASK/SDK01 package E).
#
# WHY: AAPCS64 `long double` is IEEE binary128. The hardware has no quad
# arithmetic, so clang lowers every `long double` operation to a soft-float
# libcall (__addtf3, __multf3, __divtf3, __lttf2, __extendsftf2, __trunctfdf2,
# ...), and C99 complex `*` and `/` to __mulsc3/__muldc3/__divsc3/__divdc3.
# Until this archive existed the SDK linked no compiler runtime, so OpenLibm's
# powf, remainder, fdim, nexttowardf, cpowf, casin, cacos, catanh ... and
# mintlib's qecvt/qfcvt/qgcvt did not link (docs/sdk/07-mintlib-limits.md
# section 11, docs/sdk/05-abi-pitfalls.md B12). This is the same runtime a
# hosted clang toolchain links implicitly; the SDK links it explicitly, last
# in the archive group (sdk/bin/newtos-cc).
#
# SOURCE PIN: sdk/compiler-rt/compiler-rt-21.1.8.src.tar.xz, the upstream LLVM
# 21.1.8 compiler-rt tarball, unchanged; provenance, signature check and
# SHA-256 in compiler-rt-sources.sha256. License Apache-2.0 WITH
# LLVM-exception (docs/sdk/09-port-material.md). No overlay: every source is
# compiled as shipped.
#
# WHAT IS COMPILED -- compiler-rt's own manifest, lib/builtins/CMakeLists.txt,
# for arch aarch64 in a bare-metal build (COMPILER_RT_BAREMETAL_BUILD=ON), read
# from the file, not copied by hand:
#   GENERIC_TF_SOURCES   binary128 arithmetic, comparisons, conversions,
#                        __multc3/__divtc3, __powitf2
#   GENERIC_SOURCES      the base list (integer helpers, float/double helpers,
#                        __mulsc3/__muldc3/__divsc3/__divdc3, ...), first
#                        set() only -- the conditional additions below are
#                        decided here
#   BF16_SOURCES         CMake adds them when the compiler has __bf16 (clang
#                        for AArch64 has it)
#   aarch64/fp_mode.c    of aarch64_SOURCES: __fe_getround/__fe_raise_inexact
#                        over FPCR/FPSR (EL0-accessible), so quad arithmetic
#                        honours the rounding mode fesetround() set; it
#                        replaces the generic fp_mode.c (CMake's
#                        filter_builtin_sources rule)
# NOT compiled, with the reason:
#   emutls.c enable_execute_stack.c eprintf.c
#                        CMake leaves them out of a bare-metal build (they
#                        need pthreads / mprotect / stderr)
#   atomic.c             CMake leaves it out by default
#                        (COMPILER_RT_EXCLUDE_ATOMIC_BUILTIN=On)
#   gcc_personality_v0.c an exception personality routine: calls _Unwind_*,
#                        and the SDK has no unwinder (no C++ runtime)
#   clear_cache.c        on AArch64 reads CTR_EL0 and issues DC CVAU / IC IVAU
#                        from EL0; the NewTOS kernel does not set
#                        SCTLR_EL1.UCT/UCI (kernel/src/arch/aarch64/boot.rs
#                        SCTLR_EL1_INIT), so __clear_cache would trap.
#                        Left an undefined symbol instead of a crash
#   cpu_model/aarch64.c  function-multiversioning CPU detection; reads ID
#                        registers / HWCAPs, used only by target_clones/FMV
#   aarch64/emupac.cpp   C++ (pointer-authentication emulation)
#   aarch64/lse.S        outline-atomics helpers; need cpu_model's LSE flag,
#                        and clang does not emit them for this target
#   aarch64/sme-*        SME ABI routines; the Cortex-A72 has no SME
#
# FLAGS: the SDK's freestanding set (sdk/vendor-lib.sh VL_CFLAGS: -target
# aarch64-unknown-none-elf -ffreestanding -O2 -fPIE -fno-builtin -nostdinc)
# plus compiler-rt's own for a standalone builtins build: -std=c11
# (C_STANDARD 11), -fvisibility=hidden -DVISIBILITY_HIDDEN
# (COMPILER_RT_BUILTINS_HIDE_SYMBOLS=ON), -fomit-frame-pointer,
# -DCOMPILER_RT_HAS_FLOAT16 (clang has _Float16 on AArch64). Not taken:
# -Werror=builtin-declaration-mismatch (CMake adds it only when the compiler
# knows it -- GCC; clang rejects it as an unknown warning option). Headers: clang's freestanding ones only (<float.h>
# <limits.h> <stdbool.h> <stdint.h>); no mintlib, so the archive depends on no
# other SDK library and nothing in it calls libc.
#
# Usage:  sdk/compiler-rt/build-compiler-rt.sh   (COMPILER_RT_REFRESH=1 forces a re-extract)
# Output: sdk/compiler-rt/libclang_rt.builtins.a
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/compiler-rt"
VER="21.1.8"
TOP="$LIBDIR/compiler-rt-$VER.src"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libclang_rt.builtins.a"
CMAKELISTS="$TOP/lib/builtins/CMakeLists.txt"
mkdir -p "$BUILD"

. "$SDK/vendor-lib.sh"

vl_extract "$LIBDIR/compiler-rt-$VER.src.tar.xz" "$LIBDIR/compiler-rt-sources.sha256" \
           "$TOP" "compiler-rt-$VER.src" "${COMPILER_RT_REFRESH:-0}" || exit 1
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1
vl_toolchain || exit 1

# cmake_list <NAME> -- the entries of the FIRST `set(NAME` block of the
# builtins CMakeLists.txt, one per line (the later `set(NAME ${NAME} ...)`
# blocks are the conditional additions decided in the header).
cmake_list() {
  awk -v want="set($1" '
    !inside && $0 == want { inside = 1; next }
    inside && /^[[:space:]]*\)/ { exit }
    inside { gsub(/[[:space:]]+/, ""); if ($0 != "") print }
  ' "$CMAKELISTS"
}

SRCS=()
for list in GENERIC_TF_SOURCES GENERIC_SOURCES BF16_SOURCES; do
  n=0
  while IFS= read -r f; do
    case "$f" in
      fp_mode.c) continue ;;          # replaced by aarch64/fp_mode.c (see header)
      *.c) ;;
      *) echo "error: unexpected entry '$f' in $list of $CMAKELISTS" >&2; exit 1 ;;
    esac
    SRCS[${#SRCS[@]}]="lib/builtins/$f"; n=$((n+1))
  done < <(cmake_list "$list")
  [ "$n" -gt 0 ] || { echo "error: could not read $list from $CMAKELISTS" >&2; exit 1; }
  echo "$list: $n files"
done
SRCS[${#SRCS[@]}]="lib/builtins/aarch64/fp_mode.c"
printf '%s\n' "${SRCS[@]}" > "$BUILD/srcs.txt"

LIB_CFLAGS=(-std=c11 -fvisibility=hidden -DVISIBILITY_HIDDEN -fomit-frame-pointer
            -DCOMPILER_RT_HAS_FLOAT16)
LIB_INCS=(-I"$TOP/lib/builtins")
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1
vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1

# The archive must stand alone: a reference to anything it does not define
# (a libc or libm function) would make the link order matter and could pull a
# libc member into a program that did not ask for one.
if [ -s "$BUILD/external.txt" ]; then
  echo "error: libclang_rt.builtins.a references symbols it does not define:" >&2
  sed 's/^/    /' "$BUILD/external.txt" >&2
  exit 1
fi
echo "self-contained: no external references"
