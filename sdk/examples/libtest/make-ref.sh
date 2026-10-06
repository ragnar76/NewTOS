#!/usr/bin/env bash
#
# make-ref.sh -- host side of LIBTEST (TASK/LDG-M2): build the SAME pinned
# library sources (zlib, libpng, libjpeg, giflib, libexif -- the trees the
# target build scripts extracted, with the same overlay/config headers and
# the same per-library source lists, read from their build/compiled.txt) with
# the host compiler, then build libtest.c for the host and
#
#   make-ref.sh            regenerate libtest-ref.h (host_refs[]): libtest.c
#                          with -DLIBTEST_EMIT prints every result; a result
#                          that disagrees with a Python reference
#                          (libtest-data.h) or a failing self-check aborts --
#                          a host defect must not become a reference.
#   make-ref.sh --check    regenerate into a temp file and require it to be
#                          identical to the committed libtest-ref.h, then run
#                          the normal (non-emit) host libtest against both
#                          reference tables. Exit 0 = pass. This is the SDK
#                          host test (sdk/run-host-tests.sh runs it).
#
# Why a host build is the reference for JPEG: see gen-data.py (Pillow decodes
# with libjpeg-turbo, which upsamples chroma differently from IJG 9). For the
# lossless formats the host build is a second reference next to Python's.
# The host should be LP64 little-endian (macOS/arm64 here; on arm64 libpng's
# NEON filters are compiled on both sides, as on the target).
#
# Requires: the five target libraries built once (this script runs lib_ensure
# for them, which extracts the trees), a host cc. Scratch objects go to
# sdk/examples/libtest/build/host (gitignored).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK="$(cd "$HERE/../.." && pwd)"
CC="${CC:-cc}"
MODE="${1:-}"
OUT="$HERE/build/host"
mkdir -p "$OUT"

. "$SDK/lib-stale.sh"
for l in zlib libpng libjpeg giflib libexif; do lib_ensure "$l" >&2 || exit 1; done

ZTOP="$SDK/zlib/zlib-1.3.2"
PTOP="$SDK/libpng/libpng-1.6.59"
JTOP="$SDK/libjpeg/jpeg-9f"
GTOP="$SDK/giflib/giflib-6.1.3"
ETOP="$SDK/libexif/libexif-0.6.26"

hostlib() { # hostlib <name> <top> <compiled.txt> <cflags...>
  local name=$1 top=$2 list=$3; shift 3
  local d="$OUT/$name" c o
  rm -rf "$d"; mkdir -p "$d"
  [ -s "$list" ] || { echo "error: $list missing/empty (target build of $name did not run)" >&2; return 1; }
  while IFS= read -r c; do
    o="$d/$(printf '%s' "${c%.c}" | tr '/' '_').o"
    "$CC" -O2 -w "$@" -c "$top/$c" -o "$o" || { echo "error: host compile of $name/$c failed" >&2; return 1; }
  done < "$list"
  rm -f "$OUT/lib$name.a"
  ar cr "$OUT/lib$name.a" "$d"/*.o || return 1
}

hostlib z    "$ZTOP" "$SDK/zlib/build/compiled.txt"    -I"$ZTOP"                     || exit 1
hostlib png  "$PTOP" "$SDK/libpng/build/compiled.txt"  -I"$PTOP" -I"$ZTOP"           || exit 1
hostlib jpeg "$JTOP" "$SDK/libjpeg/build/compiled.txt" -I"$JTOP"                     || exit 1
hostlib gif  "$GTOP" "$SDK/giflib/build/compiled.txt"  -std=gnu99 -I"$GTOP"          || exit 1
hostlib exif "$ETOP" "$SDK/libexif/build/compiled.txt" -I"$ETOP"                     || exit 1

INCS=(-I"$HERE" -I"$ZTOP" -I"$PTOP" -I"$JTOP" -I"$GTOP" -I"$ETOP")
LIBS=("$OUT/libpng.a" "$OUT/libz.a" "$OUT/libjpeg.a" "$OUT/libgif.a" "$OUT/libexif.a" -lm)

"$CC" -O2 -Wall -DLIBTEST_HOST -DLIBTEST_EMIT "${INCS[@]}" "$HERE/libtest.c" "${LIBS[@]}" \
      -o "$OUT/libtest-emit" || exit 1
( cd "$OUT" && ./libtest-emit ) > "$OUT/libtest-ref.h"; rc=$?
if [ $rc -ne 0 ]; then
  echo "error: host reference run reported failures (see the FAIL comments):" >&2
  grep FAIL "$OUT/libtest-ref.h" >&2
  exit 1
fi

if [ "$MODE" = "--check" ]; then
  if ! cmp -s "$OUT/libtest-ref.h" "$HERE/libtest-ref.h"; then
    echo "FAIL: host references differ from the committed libtest-ref.h:" >&2
    diff "$HERE/libtest-ref.h" "$OUT/libtest-ref.h" >&2
    exit 1
  fi
  "$CC" -O2 -Wall -DLIBTEST_HOST "${INCS[@]}" "$HERE/libtest.c" "${LIBS[@]}" -o "$OUT/libtest" || exit 1
  ( cd "$OUT" && ./libtest ) > "$OUT/libtest.log"; rc=$?
  tail -1 "$OUT/libtest.log"
  [ $rc -eq 0 ] || { grep FAIL "$OUT/libtest.log" >&2; exit 1; }
  exit 0
fi

cp "$OUT/libtest-ref.h" "$HERE/libtest-ref.h"
echo "wrote $HERE/libtest-ref.h ($(grep -c '^  { "' "$HERE/libtest-ref.h") host references)"
