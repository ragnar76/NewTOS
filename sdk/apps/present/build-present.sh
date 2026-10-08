#!/usr/bin/env bash
#
# build-present.sh -- build PRESENT.NAP, the NewTOS slide-show program
# (TASK/PRESENT01). Our own GEM application, not a port: one source file,
# sdk/apps/present/present.c, linked statically against gemlib, libpng, zlib,
# OpenLibm and mintlib -- the archives and flags of
# sdk/examples/libtest/build-libtest.sh (codec libraries) plus the gemlib
# include of sdk/apps/chaintest/build-chaintest.sh (docs/sdk/03, docs/sdk/11).
#
# Usage:  sdk/apps/present/build-present.sh [output.NAP]
#   output.NAP defaults to sdk/apps/present/build/PRESENT.NAP
#
# Slide file format and keys: docs/sdk/14-present.md and the header comment
# of present.c. The sample slides + test picture: sdk/apps/present/sample/
# (gen-sample.py writes build/TEST.PNG).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # sdk/apps/present
SDK="$(cd "$HERE/../.." && pwd)"                          # sdk
BUILD="$HERE/build"
OUT="${1:-$BUILD/PRESENT.NAP}"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
for l in gemlib zlib libpng; do lib_ensure "$l" || exit 1; done

MINT="$SDK/libc/mintlib/include"
GEMTOP="$SDK/gemlib/gemlib-0.44.0"
LIBMTOP="$SDK/libm/openlibm-0.8.8"
ZTOP="$SDK/zlib/zlib-1.3.2"
PTOP="$SDK/libpng/libpng-1.6.59"
for d in "$MINT" "$GEMTOP" "$LIBMTOP" "$ZTOP" "$PTOP"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done

CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
NM="$BIN/llvm-nm"
READOBJ="$BIN/llvm-readobj"
[ -x "$LLD" ] || { echo "error: missing LLVM tool: $LLD" >&2; exit 1; }

# The app flag set (build-libtest.sh / build-chaintest.sh).
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__
        -Wno-macro-redefined -Wall -Wextra)
# gemlib top first (gem.h), then the codec headers, mintlib, OpenLibm, clang.
INCS=(-I"$GEMTOP" -I"$ZTOP" -I"$PTOP" -I"$MINT"
      -I"$SDK/libm/include" -I"$LIBMTOP/include" -isystem "$RES")

rm -f "$BUILD"/*.o "$OUT"
if ! "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/present.c" -o "$BUILD/present.o" 2>"$BUILD/present.err"; then
  echo "error: present.c did not compile:" >&2; sed 's/^/    /' "$BUILD/present.err" >&2; exit 1
fi
if [ -s "$BUILD/present.err" ]; then echo "  warnings in present.c:"; sed 's/^/      /' "$BUILD/present.err"; fi
"$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc \
    -I"$SDK/include" -I"$MINT" -c "$SDK/libc/crt0.S" -o "$BUILD/crt0.o" || exit 1

if ! "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none "$BUILD/crt0.o" "$BUILD/present.o" \
    --start-group "$SDK/gemlib/gemlib.a" "$SDK/libpng/libpng.a" "$SDK/zlib/libz.a" \
                  "$SDK/libm/libm.a" "$SDK/libc/libc.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"; then
  echo "error: link of PRESENT.NAP failed:" >&2; sed 's/^/    /' "$BUILD/link.err" >&2; exit 1
fi
echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
"$READOBJ" --file-headers "$OUT" | grep -E '^ *Type:'
echo "  undefined symbols (goal 0): $("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)"
echo "  relocation types: $("$READOBJ" -r "$OUT" | awk '/R_AARCH64/{print $2}' | sort | uniq -c | tr '\n' ' ')"

# The sample test picture (deterministic) beside the program.
python3 "$HERE/sample/gen-sample.py" "$BUILD" || { echo "error: gen-sample.py failed" >&2; exit 1; }
