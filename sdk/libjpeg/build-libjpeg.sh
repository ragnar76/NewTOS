#!/usr/bin/env bash
#
# build-libjpeg.sh -- build the Independent JPEG Group's libjpeg (IJG
# license) into a NewTOS/AArch64 static library sdk/libjpeg/libjpeg.a
# (TASK/LDG-M2). Needed by zView's JPEG codec (plugins/jpg/Makefile: -ljpeg
# -lexif -lm; jpgldg.c decodes through jpeg_stdio_src and its own source
# manager, saves COM/APP1 markers for EXIF, and encodes through
# jpeg_stdio_dest). plugins/libs.txt names http://www.ijg.org/ as the source.
#
# SOURCE PIN: sdk/libjpeg/jpegsrc.v9f.tar.gz, IJG release 9f (origin and hash
# in libjpeg-sources.sha256). The codec uses only the classic v6b-era API, so
# any IJG release fits; 9f is the current one.
#
# What is compiled: IJG's own library list, Makefile.am
#   LIBSOURCES = jaricom.c ... jmemmgr.c @MEMORYMGR@.c
# with @MEMORYMGR@ = jmemnobs (configure.ac: "If no -enable-maxmem, use
# jmemnobs" -- malloc/free, no temporary files). All sources unmodified.
#
# Overlay (sdk/libjpeg/overlay/):
#   jconfig.h   configure's output for this target, written by hand from
#               jconfig.cfg, every answer commented (CHAR_IS_UNSIGNED on
#               AArch64, jmemnobs, ...)
#
# Usage:  sdk/libjpeg/build-libjpeg.sh   (LIBJPEG_REFRESH=1 forces a re-extract)
# Output: sdk/libjpeg/libjpeg.a; headers for programs: -I sdk/libjpeg/jpeg-9f
#         (jpeglib.h, jconfig.h, jmorecfg.h, jerror.h); programs include
#         <stdio.h> before <jpeglib.h> (IJG rule)
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/libjpeg"
VER="9f"
TOP="$LIBDIR/jpeg-$VER"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libjpeg.a"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
lib_ensure libc || exit 1
MINT="$SDK/libc/mintlib/include"
[ -d "$MINT" ] || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }

vl_extract "$LIBDIR/jpegsrc.v$VER.tar.gz" "$LIBDIR/libjpeg-sources.sha256" "$TOP" "jpeg-$VER" "${LIBJPEG_REFRESH:-0}" || exit 1
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1

vl_toolchain || exit 1

# Source list read from IJG's own Makefile.am (LIBSOURCES, continuation lines),
# @MEMORYMGR@ resolved to configure's default.
srcs="$(awk '/^LIBSOURCES = /{f=1} f{print; if ($0 !~ /\\$/) exit}' "$TOP/Makefile.am" \
        | sed -e 's/^LIBSOURCES = //' -e 's/@MEMORYMGR@/jmemnobs/' | tr ' \t\\' '\n\n\n' | grep '\.c$')"
[ -n "$srcs" ] || { echo "error: could not read LIBSOURCES from $TOP/Makefile.am" >&2; exit 1; }
SRCS=()
for c in $srcs; do SRCS[${#SRCS[@]}]="$c"; done

# -Wall, not -W -Wall: IJG's own build sets no warning flags at all, and -W
# adds only -Wunused-parameter (42 hits, all the IJG method-table signatures
# whose implementations ignore an argument, e.g. jmemnobs.c). -Wall is clean.
LIB_CFLAGS=(-Wall)
LIB_INCS=(-I"$TOP" -I"$MINT")
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1
vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1
