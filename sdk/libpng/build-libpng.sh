#!/usr/bin/env bash
#
# build-libpng.sh -- build libpng (PNG Reference Library License v2) into a
# NewTOS/AArch64 static library sdk/libpng/libpng.a (TASK/LDG-M2). Needed by
# zView's PNG codec (plugins/png/Makefile: -lpng -lm -lz; pngldg.c uses the
# classic read API: png_create_read_struct, png_init_io, png_read_info,
# png_set_expand/strip_16/gray_to_rgb/interlace_handling, png_read_row,
# png_get_text) and by zView itself (zview/Makefile: -lpng -lz).
#
# SOURCE PIN: sdk/libpng/libpng-1.6.59.tar.xz, upstream release (origin and
# hashes in libpng-sources.sha256). Any libpng 1.6 provides the API the codec
# uses; 1.6.59 is the current release.
#
# What is compiled: libpng's own source list, Makefile.am
#   libpng16_la_SOURCES = png.c pngerror.c pngget.c pngmem.c pngpread.c
#     pngread.c pngrio.c pngrtran.c pngrutil.c pngset.c pngtrans.c pngwio.c
#     pngwrite.c pngwtran.c pngwutil.c
#   + (if PNG_ARM_NEON) arm/arm_init.c arm/filter_neon_intrinsics.c
#     arm/palette_neon_intrinsics.c
# The NEON part is upstream's DEFAULT on AArch64: pngpriv.h sets
# PNG_ARM_NEON_OPT=2 (compile-time on, no run-time CPU check -- arm_init.c
# forbids PNG_ARM_NEON_CHECK_SUPPORTED on ARM64) when __ARM_NEON is defined,
# which clang does for aarch64-unknown-none-elf. EL0 programs use FP/SIMD
# already (libm), so nothing is switched off here.
#
# pnglibconf.h is upstream's scripts/pnglibconf.h.prebuilt, copied unchanged
# -- the step INSTALL ("cp scripts/pnglibconf.h.prebuilt pnglibconf.h")
# prescribes for builds without configure. It enables the full default
# feature set incl. STDIO (png_init_io) and SETJMP (png_jmpbuf), both of
# which the codec uses and mintlib provides. All sources unmodified; no
# overlay.
#
# Usage:  sdk/libpng/build-libpng.sh     (LIBPNG_REFRESH=1 forces a re-extract)
# Output: sdk/libpng/libpng.a; headers for programs:
#         -I sdk/libpng/libpng-1.6.59 (png.h, pngconf.h, pnglibconf.h)
#         -I sdk/zlib/zlib-1.3.2      (png.h includes zlib.h)
#         (libpng itself is compiled against OpenLibm's math.h as well)
#         link: libpng.a libz.a libm.a libc.a
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/libpng"
VER="1.6.59"
TOP="$LIBDIR/libpng-$VER"
ZTOP="$SDK/zlib/zlib-1.3.2"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libpng.a"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
lib_ensure zlib || exit 1
lib_ensure libm || exit 1          # pngpriv.h includes <math.h> (floating-point gamma)
MINT="$SDK/libc/mintlib/include"
LIBMTOP="$SDK/libm/openlibm-0.8.8"
for d in "$MINT" "$ZTOP" "$LIBMTOP"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done

vl_extract "$LIBDIR/libpng-$VER.tar.xz" "$LIBDIR/libpng-sources.sha256" "$TOP" "libpng-$VER" "${LIBPNG_REFRESH:-0}" || exit 1
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1
cp "$TOP/scripts/pnglibconf.h.prebuilt" "$TOP/pnglibconf.h" || exit 1

vl_toolchain || exit 1

# Source list read from libpng's own Makefile.am: the base list, then the
# PNG_ARM_NEON block (see header).
am="$TOP/Makefile.am"
base="$(awk '/^libpng@PNGLIB_MAJOR@@PNGLIB_MINOR@_la_SOURCES = /{f=1} f{print; if ($0 !~ /\\$/) exit}' "$am" \
        | tr ' \t\\' '\n\n\n' | grep '\.c$')"
neon="$(awk '/^if PNG_ARM_NEON$/{f=1;next} f&&/^endif/{exit} f{print}' "$am" \
        | tr ' \t\\' '\n\n\n' | grep '\.c$')"
[ -n "$base" ] && [ -n "$neon" ] || { echo "error: could not read the source lists from $am" >&2; exit 1; }
SRCS=()
for c in $base $neon; do SRCS[${#SRCS[@]}]="$c"; done

# -Wno-macro-redefined: pngpriv.h includes both <float.h> (clang's, which
# defines INFINITY/NAN for C23) and OpenLibm's <math.h> (which defines them
# again, equivalently). The same clash is why the app builds pass this flag
# (sdk/ldg/build-ldg-demo.sh, build-qed.sh); it is a header-pair artefact, not
# a libpng finding.
LIB_CFLAGS=(-W -Wall -Wno-macro-redefined)
LIB_INCS=(-I"$TOP" -I"$ZTOP" -I"$MINT" -I"$SDK/libm/include" -I"$LIBMTOP/include")
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1
vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1
