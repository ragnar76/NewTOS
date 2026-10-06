#!/usr/bin/env bash
#
# build-zlib.sh -- build zlib (Jean-loup Gailly & Mark Adler, zlib license)
# into a NewTOS/AArch64 static library sdk/zlib/libz.a (TASK/LDG-M2).
# Needed by zView's PNG codec (plugins/png/Makefile: -lpng -lm -lz) and by
# libpng itself.
#
# SOURCE PIN: sdk/zlib/zlib-1.3.2.tar.xz, the upstream release from
# https://zlib.net/zlib-1.3.2.tar.xz (detached signature zlib-1.3.2.tar.xz.asc
# from the same place, verified 2026-10-06 against Mark Adler's key
# 5ED46A6721D365587791E2AA783FCD8E58BCAFBA). SHA-256 in zlib-sources.sha256.
# z-tools/prerequisites.txt names zlib 1.2.8; 1.3.2 is the current release and
# keeps that API (zView/libpng only use compress/inflate/crc32 entry points).
#
# What is compiled: zlib's own object list, Makefile.in
#   OBJZ = adler32 crc32 deflate infback inffast inflate inftrees trees zutil
#   OBJG = compress uncompr gzclose gzlib gzread gzwrite
# All sources unmodified. zconf.h is GENERATED from the pristine zconf.h.in
# with exactly the two edits zlib's ./configure makes on a target that has
# <unistd.h> and <stdarg.h> (configure, "check for unistd.h"/"check for
# stdarg.h": `#if HAVE_UNISTD_H-0 ... may be` -> `#if 1 ... was`). mintlib has
# both; with Z_HAVE_UNISTD_H z_off_t is mintlib's off_t (32-bit, TOS file
# offsets) for the library and every program that includes zlib.h alike.
# No overlay.
#
# Usage:  sdk/zlib/build-zlib.sh     (ZLIB_REFRESH=1 forces a re-extract)
# Output: sdk/zlib/libz.a; headers for programs: -I sdk/zlib/zlib-1.3.2
#         (zlib.h, zconf.h)
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/zlib"
VER="1.3.2"
TOP="$LIBDIR/zlib-$VER"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libz.a"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
lib_ensure libc || exit 1
MINT="$SDK/libc/mintlib/include"
[ -d "$MINT" ] || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }

vl_extract "$LIBDIR/zlib-$VER.tar.xz" "$LIBDIR/zlib-sources.sha256" "$TOP" "zlib-$VER" "${ZLIB_REFRESH:-0}" || exit 1
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1

# zconf.h from zconf.h.in, configure's two seds verbatim (see header).
sed -e "/^#if HAVE_UNISTD_H-0.* may be/s/ HAVE_UNISTD_H-0\(.*\) may be/ 1\1 was/" \
    -e "/^#if HAVE_STDARG_H-0.* may be/s/ HAVE_STDARG_H-0\(.*\) may be/ 1\1 was/" \
    "$TOP/zconf.h.in" > "$TOP/zconf.h"
grep -q '^#if 1 .*was set to #if 1 by ./configure' "$TOP/zconf.h" ||
  { echo "error: zconf.h generation did not apply (zconf.h.in changed?)" >&2; exit 1; }
[ "$(grep -c 'was set to #if 1 by ./configure' "$TOP/zconf.h")" -eq 2 ] ||
  { echo "error: expected exactly 2 configure edits in zconf.h" >&2; exit 1; }

vl_toolchain || exit 1

# Source list read from zlib's own Makefile.in (OBJZ + OBJG).
objs="$(sed -n 's/^OBJ[ZG][[:space:]]*=[[:space:]]*//p' "$TOP/Makefile.in" | tr ' ' '\n' | grep '\.o$')"
[ -n "$objs" ] || { echo "error: could not read OBJZ/OBJG from $TOP/Makefile.in" >&2; exit 1; }
SRCS=()
for o in $objs; do SRCS[${#SRCS[@]}]="${o%.o}.c"; done

LIB_CFLAGS=(-W -Wall)
LIB_INCS=(-I"$TOP" -I"$MINT")
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1
vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1
