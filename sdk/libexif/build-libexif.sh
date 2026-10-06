#!/usr/bin/env bash
#
# build-libexif.sh -- build libexif (Lutz Mueller et al., LGPL 2.1 or later)
# into a NewTOS/AArch64 static library sdk/libexif/libexif.a (TASK/LDG-M2).
# Needed by zView's JPEG codec (plugins/jpg/Makefile: -ljpeg -lexif -lm;
# jpgldg.c:374-458 parses the APP1 marker with exif_data_new_from_data,
# walks the entries and formats them with exif_tag_get_name /
# exif_entry_get_value for the image-info text, exif_data_unref).
#
# SOURCE PIN: sdk/libexif/libexif-0.6.26.tar.xz, upstream release (origin,
# signature check and hashes in libexif-sources.sha256). The four calls the
# codec makes have been stable since 0.6.x; 0.6.26 is the current release.
#
# What is compiled: libexif's own lists, libexif/Makefile.am
#   libexif_la_SOURCES (exif-byte-order.c ... exif-gps-ifd.c)
#   + the four maker-note convenience libraries it links in, from
#     canon/ fuji/ olympus/ pentax/ Makefile-files (libmnote-*_la_SOURCES)
#   (apple/ is NOT built upstream either: Makefile.am comments its include
#   out, "apple makernote currently cannot save itself").
# Flags: libexif's own warning set (configure.ac:288, GCC branch:
# -Wall -Wmissing-declarations -Wmissing-prototypes) and its include dirs
# (top_srcdir, top_builddir). All sources unmodified.
#
# Overlay (sdk/libexif/overlay/):
#   config.h   configure's output for this target, written by hand from
#              config.h.in, every answer commented (no NLS, localtime_r, ...)
#
# libexif calls sqrt/pow/fabs (exif-entry.c: rational formatting), so it is
# compiled against OpenLibm's math.h and programs link libm.a.
#
# Usage:  sdk/libexif/build-libexif.sh   (LIBEXIF_REFRESH=1 forces a re-extract)
# Output: sdk/libexif/libexif.a; headers for programs:
#         -I sdk/libexif/libexif-0.6.26 (#include <libexif/exif-data.h>)
#         link: libexif.a libm.a libc.a
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/libexif"
VER="0.6.26"
TOP="$LIBDIR/libexif-$VER"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libexif.a"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
lib_ensure libm || exit 1
MINT="$SDK/libc/mintlib/include"
LIBMTOP="$SDK/libm/openlibm-0.8.8"
for d in "$MINT" "$LIBMTOP"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done

vl_extract "$LIBDIR/libexif-$VER.tar.xz" "$LIBDIR/libexif-sources.sha256" "$TOP" "libexif-$VER" "${LIBEXIF_REFRESH:-0}" || exit 1
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1

vl_toolchain || exit 1

# Source lists read from libexif's own automake files (see header). Paths in
# libexif/Makefile.am are relative to libexif/.
am_list() { # am_list <file> <variable>  -- .c entries of a (continued) variable
  awk -v v="$2" '$0 ~ "^"v"[[:space:]]*=" {f=1} f{print; if ($0 !~ /\\$/) exit}' "$1" \
    | sed -e "s/^$2[[:space:]]*=//" | tr ' \t\\' '\n\n\n' | grep '\.c$'
}
main="$(am_list "$TOP/libexif/Makefile.am" libexif_la_SOURCES)"
mnote=""
for m in canon fuji olympus pentax; do
  mnote="$mnote $(am_list "$TOP/libexif/$m/Makefile-files" "libmnote_${m}_la_SOURCES")"
done
[ -n "$main" ] && [ -n "$(echo $mnote)" ] || { echo "error: could not read the source lists from $TOP/libexif" >&2; exit 1; }
SRCS=()
for c in $main $mnote; do SRCS[${#SRCS[@]}]="libexif/$c"; done

# One upstream warning remains and is left visible on purpose: exif-entry.c
# :1243/1267/1280 -Wswitch "case value not in enumerated type 'ExifTag'" --
# the GPS tags are plain #defines (exif-tag.h:176-183) whose values are not
# ExifTag enumerators; the switch still compares the integer tag number, so
# it behaves as written. Upstream source, not changed here.
LIB_CFLAGS=(-Wall -Wmissing-declarations -Wmissing-prototypes)
LIB_INCS=(-I"$TOP" -I"$MINT" -I"$SDK/libm/include" -I"$LIBMTOP/include")
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1
vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1
