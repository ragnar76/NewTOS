#!/usr/bin/env bash
#
# build-libc-ilp32.sh -- build mintlib-0.60.1 into an **ILP32** NewTOS/AArch64
# libc-ilp32.a and link the libtest client into a native ELF32 program (.NCP).
# This is the GNU-ILP32 sibling of build-libc.sh (ADR-020 Stage 2, the
# LP64->ILP32 app-ABI pivot). build-libc.sh (clang/LP64) is left intact for the
# future LP64-native path; this script and its artifacts are entirely separate:
#
#     tree     : sdk/libc/mintlib-ilp32/   (vs LP64 sdk/libc/mintlib/)
#     objects  : sdk/libc/build-ilp32/     (vs LP64 sdk/libc/build/)
#     archive  : sdk/libc/libc-ilp32.a     (vs LP64 sdk/libc/libc.a)
#     crt0     : sdk/libc/crt0-ilp32.S     (vs LP64 sdk/libc/crt0.S)
#
# Toolchain (GNU bare-metal AArch64, Homebrew aarch64-elf-*):
#   - aarch64-elf-gcc -mabi=ilp32  -> 4-byte long/pointer, ELF32 objects.
#     (GCC 16 prints "-mabi=ilp32 is deprecated"; it is the only spelling and
#     works -- ignore the note.)
#   - aarch64-elf-ar               -> archive.
#   - aarch64-elf-ld -m aarch64elf32 -> ELF32 AArch64 static executable.
#
# Why several LP64 overlays are DROPPED on ILP32+gcc (pristine mintlib is now
# correct; verified by compile/link, see the deliverable notes):
#   - overlay/stdio/vfprintf.c  DROPPED. The LP64 overlay only patched the
#     is_longlong "assign to a compile-time-constant 0" case, which exists
#     solely when LONG_MAX == LONG_LONG_MAX (LP64). On ILP32 long(4) != long
#     long(8), so is_longlong is a real assignable variable and the pristine
#     file compiles as-is under gcc (computed-goto is a native gcc extension).
#
# LP64 overlays/shims KEPT on ILP32 (still required -- reason each):
#   - overlay/stdlib/labs.c     KEPT. The pristine mintlib/labs.c is
#     `strong_alias(abs, labs)`, which expands (attribute form disabled in
#     libc-symbols.h) to `__asm__(".globl labs; .set labs, abs")`. With `abs`
#     in another TU, GAS does NOT emit a usable `labs` -- verified: the object
#     exports only `U abs`, and a caller fails to link. This is a cross-TU
#     `.set`-alias limitation, NOT a clang-only issue, so the real definition
#     is still needed.
#   - overlay/include/bits/byteswap.h  KEPT. Pristine __bswap_32 uses m68k
#     ror.w/swap inline asm guarded only by `!__mcoldfire__`; on AArch64 that
#     branch is taken and will not assemble. Arch issue, not toolchain/ABI.
#   - overlay/include/mint/{osbind,mintbind,falcon,metados}.h  KEPT. The
#     retargeted register-SVC trap macros; the m68k stack-trap form is gone.
#   - overlay/include/{features,linker}.h  KEPT. Generated config headers
#     (__HAVE_GNU_LD etc.).
#   - newtos-arch/*  KEPT. Honest trap-backed OS-boundary shims
#     (memops/proc/heap_init/getcookie/do_fstat/do_stat/isatty/ioctl/access/
#     clock) replacing m68k asm; ABI-agnostic.
#   - -D__NO_LONG_DOUBLE_MATH + newtos-arch/softfloat.c (__extenddftf2)  KEPT.
#     `aarch64-elf-gcc -mabi=ilp32` keeps `long double` == 128-bit IEEE quad
#     (sizeof == 16), so the float printf/scanf engine would still emit quad
#     soft-float libcalls we have no runtime for; routing long double through
#     hardware double and supplying __extenddftf2 is still required.
#
# The linker pulls only the archive members it needs; whole core dirs are
# compiled best-effort (m68k .S / unrelated helpers that don't assemble for
# AArch64 are skipped and logged). A *needed* skip surfaces as an undefined
# symbol at link time.
#
# Usage:  sdk/libc/build-libc-ilp32.sh [client.c] [output.NCP]
#   client.c   defaults to sdk/libc/tests/libtest.c
#   output.NCP defaults to <build-ilp32>/<client>.NCP
#
# Toolchain override:
#   GCC=/path/to/aarch64-elf-gcc LD=... AR=... sdk/libc/build-libc-ilp32.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBC="$SDK/libc"
TOP="$LIBC/mintlib-ilp32"                                    # ILP32 top_srcdir
CLIENT="${1:-$LIBC/tests/libtest.c}"
BUILD="$LIBC/build-ilp32"
OUT="${2:-$BUILD/$(basename "${CLIENT%.c}").NCP}"
OBJ="$BUILD/obj"
mkdir -p "$OBJ"

# --- locate the GNU bare-metal AArch64 toolchain ------------------------------
find_tool() { # $1 = env override (maybe empty), $2 = tool basename
    local override="$1" name="$2"
    if [ -n "$override" ]; then echo "$override"; return 0; fi
    if command -v "$name" >/dev/null 2>&1; then echo "$name"; return 0; fi
    if [ -x "/opt/homebrew/bin/$name" ]; then echo "/opt/homebrew/bin/$name"; return 0; fi
    return 1
}
GCC="$(find_tool "${GCC:-}" aarch64-elf-gcc)" \
    || { echo "error: aarch64-elf-gcc not found (brew install aarch64-elf-gcc)" >&2; exit 1; }
LD="$(find_tool "${LD:-}"  aarch64-elf-ld)" \
    || { echo "error: aarch64-elf-ld not found (brew install aarch64-elf-binutils)" >&2; exit 1; }
AR="$(find_tool "${AR:-}"  aarch64-elf-ar)" \
    || { echo "error: aarch64-elf-ar not found" >&2; exit 1; }
READELF="$(find_tool "${READELF:-}" aarch64-elf-readelf)" || READELF=""

# --- ensure the ILP32 mintlib source tree is present (extract + overlay) ------
# The extracted tree is a build artifact (gitignored). The pristine tarball +
# overlay/ are the tracked inputs. MINTLIB_REFRESH=1 forces a clean re-extract.
TARBALL="$LIBC/mintlib-0.60.1.tar.xz"
OVERLAY="$LIBC/overlay"
# LP64 overlays that REVERT to pristine on ILP32+gcc (see header). Paths are
# relative to overlay/; they are NOT copied into the ILP32 tree.
OVERLAY_DROP="stdio/vfprintf.c"
is_dropped() { case " $OVERLAY_DROP " in *" $1 "*) return 0;; esac; return 1; }

if [ "${MINTLIB_REFRESH:-0}" = "1" ]; then rm -rf "$TOP"; fi
if [ ! -d "$TOP" ]; then
  [ -f "$TARBALL" ] || { echo "error: pristine tarball $TARBALL missing" >&2; exit 1; }
  echo "extracting pristine mintlib from $(basename "$TARBALL")…"
  tmp="$(mktemp -d)"
  tar xf "$TARBALL" -C "$tmp"
  mv "$tmp/mintlib-0.60.1" "$TOP"
  rmdir "$tmp" 2>/dev/null || true
fi
# Apply the NewTOS overlay, MINUS the ILP32-dropped files (pristine stays).
if [ -d "$OVERLAY" ]; then
  ( cd "$OVERLAY" && find . -type f -print0 | while IFS= read -r -d '' f; do
      rel="${f#./}"
      if is_dropped "$rel"; then continue; fi
      mkdir -p "$TOP/$(dirname "$f")"
      cp "$f" "$TOP/$f"
    done )
fi

# --- flags (mintlib buildrules/configvars, retargeted for GNU ILP32) ----------
GINC="$("$GCC" -mabi=ilp32 -print-file-name=include)"       # gcc fixed includes
CFLAGS=(-mabi=ilp32 -ffreestanding -nostdlib -O2
        -fno-pic -fno-builtin -nostdinc
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH)
INCS=(-I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$GINC")

# mintlib core directories the linker draws from.
DIRS="stdio stdlib string mintlib unix syscall time misc multibyte signal termios locale dirent gmp posix tz"

# mintlib .c files replaced by lean NewTOS glue in newtos-arch/ (identical set to
# the LP64 build; see build-libc.sh for the per-file rationale).
EXCLUDE="mintlib/do_fstat.c mintlib/isatty.c mintlib/do_stat.c unix/access.c time/clock.c"
is_excluded() { case " $EXCLUDE " in *" $1 "*) return 0;; esac; return 1; }

echo "gcc : $("$GCC" -mabi=ilp32 --version 2>/dev/null | head -1)"
echo "ar  : $AR"
echo "ld  : $("$LD" --version | head -1)"
echo "abi : ILP32 (4-byte long/pointer, ELF32 AArch64)"
echo

# --- compile mintlib core into objects ----------------------------------------
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"; : > "$BUILD/excluded.txt"
ok=0; fail=0
for d in $DIRS; do
    for c in "$TOP/$d"/*.c; do
        [ -e "$c" ] || continue
        b=$(basename "${c%.c}")
        case "$b" in test-*|*-skeleton|ftest|xpg_basename|zdump|zic|tzselect) continue;; esac
        if is_excluded "$d/$b.c"; then echo "$d/$b.c" >> "$BUILD/excluded.txt"; continue; fi
        o="$OBJ/${d}__${b}.o"
        DEFS_FILE=()
        case "$d/$b" in mintlib/ident) DEFS_FILE=(-DVERSION='"0.60.1"');; esac
        if "$GCC" "${CFLAGS[@]}" ${DEFS_FILE[@]+"${DEFS_FILE[@]}"} -I"$TOP/$d" "${INCS[@]}" -c "$c" -o "$o" 2>"$OBJ/${d}__${b}.err"; then
            echo "$d/$b.c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
        else
            echo "$d/$b.c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
        fi
    done
done

# --- NewTOS arch shims (kept verbatim from the LP64 build; ABI-agnostic) -------
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" -I"$TOP/include" -isystem "$GINC" \
    -c "$LIBC/newtos-arch/memops.c"    -o "$OBJ/arch__memops.o"   || exit 1
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" -I"$TOP/include" -isystem "$GINC" \
    -c "$LIBC/newtos-arch/heap_init.c" -o "$OBJ/arch__heapinit.o" || exit 1
"$GCC" "${CFLAGS[@]}" -I"$TOP/include" \
    -c "$LIBC/newtos-arch/proc.S"      -o "$OBJ/arch__proc.o"     || exit 1
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/do_fstat.c"  -o "$OBJ/arch__do_fstat.o" || exit 1
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/isatty.c"    -o "$OBJ/arch__isatty.o"   || exit 1
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/ioctl.c"     -o "$OBJ/arch__ioctl.o"    || exit 1
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/do_stat.c"   -o "$OBJ/arch__do_stat.o"  || exit 1
"$GCC" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/getcookie.c" -o "$OBJ/arch__getcookie.o" || exit 1
"$GCC" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/clock.c"     -o "$OBJ/arch__clock.o"     || exit 1
"$GCC" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/access.c"    -o "$OBJ/arch__access.o"    || exit 1
"$GCC" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/softfloat.c" -o "$OBJ/arch__softfloat.o" || exit 1

# --- archive ------------------------------------------------------------------
rm -f "$LIBC/libc-ilp32.a"
"$AR" cr "$LIBC/libc-ilp32.a" "$OBJ"/*.o
echo "compiled OK=$ok  skipped=$fail  -> $LIBC/libc-ilp32.a ($(wc -c < "$LIBC/libc-ilp32.a" | tr -d ' ') bytes)"
echo

# --- CRT + client -------------------------------------------------------------
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" -I"$TOP/include" -I"$TOP/mintlib" \
    -c "$LIBC/crt0-ilp32.S" -o "$BUILD/crt0.o" || exit 1
"$GCC" "${CFLAGS[@]}" -I"$TOP" "${INCS[@]}" \
    -c "$CLIENT" -o "$BUILD/client.o" || exit 1

# --- link (crt0 + client + libc-ilp32.a via the NewTOS TPA linker script) -----
# `-m aarch64elf32` selects the ELF32 AArch64 output format from newtos.ld; the
# single RWX PT_LOAD is intentional (loader maps every TPA page UserRwExec).
"$LD" -m aarch64elf32 -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --no-warn-rwx-segments -Map "$BUILD/link.map" \
    "$BUILD/crt0.o" "$BUILD/client.o" "$LIBC/libc-ilp32.a" -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    [ -n "$READELF" ] && "$READELF" -h "$OUT" | grep -iE 'Class:|Machine:|Type:|Entry'
else
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined reference to \`[^']+'" "$BUILD/link.err" \
        | sed "s/.*\`//; s/'//" | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/link.err and $BUILD/link.map for detail."
    exit 1
fi
