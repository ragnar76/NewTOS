#!/usr/bin/env bash
#
# build-giflib.sh -- build giflib (Eric S. Raymond et al., MIT license) into a
# NewTOS/AArch64 static library sdk/giflib/libgif.a (TASK/LDG-M2). Needed by
# zView's GIF codec (plugins/gif/Makefile: -lgif; gifldg.c uses the giflib 5
# decoder API: DGifOpenFileName(name, &err), DGifGetRecordType,
# DGifGetImageDesc, DGifGetLine, DGifGetExtension/-Next,
# DGifCloseFile(gif, &err) -- the two-argument forms exist since giflib 5.1).
#
# SOURCE PIN: sdk/giflib/giflib-6.1.3.tar.gz, upstream release (origin and
# hashes in giflib-sources.sha256). giflib 6 changed only EGifSpew()'s
# signature (NEWS, "Version 6.1.1"), which the codec does not call; the
# decoder entry points above are identical to 5.1/5.2 (gif_lib.h).
#
# What is compiled: giflib's own Makefile SOURCES list
#   dgif_lib.c egif_lib.c gifalloc.c gif_err.c gif_font.c gif_hash.c
#   openbsd-reallocarray.c quantize.c
# with giflib's own CFLAGS (-std=gnu99 -Wall -O2; -fPIC there is for its
# shared library, the SDK's -fPIE replaces it). HAVE_REALLOCARRAY is NOT
# defined: mintlib has no reallocarray(), so giflib's bundled
# openbsd-reallocarray.c is used, which is the Makefile's default. All
# sources unmodified; no overlay.
#
# Usage:  sdk/giflib/build-giflib.sh    (GIFLIB_REFRESH=1 forces a re-extract)
# Output: sdk/giflib/libgif.a; headers for programs: -I sdk/giflib/giflib-6.1.3
#         (gif_lib.h)
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/giflib"
VER="6.1.3"
TOP="$LIBDIR/giflib-$VER"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libgif.a"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
lib_ensure libc || exit 1
MINT="$SDK/libc/mintlib/include"
[ -d "$MINT" ] || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }

vl_extract "$LIBDIR/giflib-$VER.tar.gz" "$LIBDIR/giflib-sources.sha256" "$TOP" "giflib-$VER" "${GIFLIB_REFRESH:-0}" || exit 1
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1

vl_toolchain || exit 1

# Source list read from giflib's own Makefile (SOURCES, continuation lines).
srcs="$(awk '/^SOURCES = /{f=1} f{print; if ($0 !~ /\\$/) exit}' "$TOP/Makefile" \
        | sed -e 's/^SOURCES = //' | tr ' \t\\' '\n\n\n' | grep '\.c$')"
[ -n "$srcs" ] || { echo "error: could not read SOURCES from $TOP/Makefile" >&2; exit 1; }
SRCS=()
for c in $srcs; do SRCS[${#SRCS[@]}]="$c"; done

LIB_CFLAGS=(-std=gnu99 -Wall)
LIB_INCS=(-I"$TOP" -I"$MINT")
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1
vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1
