#!/usr/bin/env bash
#
# build-libtest.sh -- build LIBTEST.NCP, the TASK/LDG-M2 acceptance test of
# the codec libraries (zlib, libpng, libjpeg, giflib, libexif), for NewTOS.
# See libtest.c for what it checks and where its references come from
# (gen-data.py -> libtest-data.h, make-ref.sh -> libtest-ref.h).
#
# It is also the worked example of how a program links these libraries
# (docs/sdk/11-codec-libraries.md): the app flag set of build-qed.sh, the
# headers of each extracted tree, and the archives in dependency order
#   libpng.a libz.a libjpeg.a libgif.a libexif.a libm.a libc.a
# inside one --start-group/--end-group.
#
# Usage:  sdk/examples/libtest/build-libtest.sh
# Output: sdk/examples/libtest/build/LIBTEST.NCP (headless; prints
#         "libtest: ALL PASS (N checks)" over Cconws; writes and removes its
#         LT_*.TMP / LT_Z.GZ scratch files in the current directory)
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK="$(cd "$HERE/../.." && pwd)"
BUILD="$HERE/build"
OUT="$BUILD/LIBTEST.NCP"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
for l in zlib libpng libjpeg giflib libexif; do lib_ensure "$l" || exit 1; done

MINT="$SDK/libc/mintlib/include"
LIBMTOP="$SDK/libm/openlibm-0.8.8"
ZTOP="$SDK/zlib/zlib-1.3.2"
PTOP="$SDK/libpng/libpng-1.6.59"
JTOP="$SDK/libjpeg/jpeg-9f"
GTOP="$SDK/giflib/giflib-6.1.3"
ETOP="$SDK/libexif/libexif-0.6.26"
for d in "$MINT" "$LIBMTOP" "$ZTOP" "$PTOP" "$JTOP" "$GTOP" "$ETOP"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done

CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
. "$SDK/toolchain.sh"   # rustc asked from inside sdk/, whatever the cwd (SDK01 I)
BIN="$(newtos_rust_bindir)"
LLD="${LLD:-$BIN/rust-lld}"
NM="$BIN/llvm-nm"
READOBJ="$BIN/llvm-readobj"
[ -x "$LLD" ] || { echo "error: missing LLVM tool: $LLD" >&2; exit 1; }

# The app flag set (sdk/apps/build-qed.sh, sdk/ldg/build-ldg-demo.sh).
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__
        -Wno-macro-redefined -Wall)
INCS=(-I"$HERE" -I"$ZTOP" -I"$PTOP" -I"$JTOP" -I"$GTOP" -I"$ETOP"
      -I"$MINT" -I"$SDK/libm/include" -I"$LIBMTOP/include" -isystem "$RES")

rm -f "$BUILD"/*.o "$OUT"
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/libtest.c" -o "$BUILD/libtest.o" 2>"$BUILD/libtest.err" ||
  { echo "error: libtest.c did not compile:" >&2; sed 's/^/    /' "$BUILD/libtest.err" >&2; exit 1; }
if [ -s "$BUILD/libtest.err" ]; then echo "  warnings in libtest.c:"; sed 's/^/      /' "$BUILD/libtest.err"; fi
"$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc \
    -I"$SDK/include" -I"$MINT" -c "$SDK/libc/crt0.S" -o "$BUILD/crt0.o" || exit 1

if ! "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none "$BUILD/crt0.o" "$BUILD/libtest.o" \
    --start-group "$SDK/libpng/libpng.a" "$SDK/zlib/libz.a" "$SDK/libjpeg/libjpeg.a" \
                  "$SDK/giflib/libgif.a" "$SDK/libexif/libexif.a" \
                  "$SDK/libm/libm.a" "$SDK/libc/libc.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"; then
  echo "error: link of LIBTEST.NCP failed:" >&2; sed 's/^/    /' "$BUILD/link.err" >&2; exit 1
fi
echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
"$READOBJ" --file-headers "$OUT" | grep -E '^ *Type:'
echo "  undefined symbols (goal 0): $("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)"
echo "  relocation types: $("$READOBJ" -r "$OUT" | awk '/R_AARCH64/{print $2}' | sort | uniq -c | tr '\n' ' ')"
