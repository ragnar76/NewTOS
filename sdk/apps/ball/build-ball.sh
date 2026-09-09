#!/usr/bin/env bash
#
# build-ball.sh -- build BALL.NAP, the NewTOS bouncing-ball demo, for
# NewTOS/AArch64 as a native LP64 GEM application (ADR-018 `.NAP`).
#
# Modelled on sdk/apps/gemtest/build-gemtest.sh and
# tosdump/build-vdidraw-newtos.sh: the same toolchain, headers and ABI every
# other native GEM client here uses -- clang -target aarch64-unknown-none-elf,
# the libc-aware crt0, the ported mintlib (libc.a) and gemlib (gemlib.a),
# linked with sdk/newtos.ld.
#
#   ball.c         the machine-independent demo (physics, palette, the sphere
#                  map and its renderer, dirty rectangles, main loop) --
#                  compiled unchanged for a future m68k/TT build
#   ball-tex.c     the ball's texture: the NTX1 file reader and, when there is
#                  no file, the generator -- machine-independent too
#   plat-newtos.c  the Pi-4 half: VDI geometry, the pitch measurement, the
#                  screen pages, the index->XRGB blit
#
# It is a `.NAP` because it opens a VDI workstation the way a GEM application
# does (appl_init / graf_handle / v_opnvwk), so it launches from the desktop
# -- never from C:\AUTO, which runs console programs only.
#
# -O2, not -Os: this one is a performance demo.
#
#   objs   : sdk/apps/ball/build/
#   output : sdk/apps/ball/BALL.NAP  (ELF64 AArch64)
#
# Usage:  sdk/apps/ball/build-ball.sh [output.NAP]
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"      # sdk/apps/ball
SDK="$(cd "$HERE/../.." && pwd)"                          # sdk
OUT="${1:-$HERE/BALL.NAP}"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
GEMTOP="$GEM/gemlib-0.44.0"
BUILD="$HERE/build"
mkdir -p "$BUILD"

for s in ball.c ball-tex.c plat-newtos.c ball.h ball-tex.h; do
    [ -f "$HERE/$s" ] || { echo "error: $HERE/$s missing" >&2; exit 1; }
done

# --- the two ported libraries -------------------------------------------------
if [ ! -f "$LIBC/libc.a" ]; then
  echo "libc.a absent -- running build-libc.sh first…"
  "$LIBC/build-libc.sh" >/dev/null 2>&1 || { echo "error: build-libc.sh failed" >&2; exit 1; }
fi
if [ ! -f "$GEM/gemlib.a" ]; then
  echo "gemlib.a absent -- running build-gemlib.sh first…"
  "$GEM/build-gemlib.sh" >/dev/null 2>&1 || { echo "error: build-gemlib.sh failed" >&2; exit 1; }
fi
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ]   || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }
[ -d "$GEMTOP" ] || { echo "error: gemlib headers missing ($GEMTOP); run build-gemlib.sh" >&2; exit 1; }

# --- toolchain ----------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
READOBJ="$BIN/llvm-readobj"
NM="$BIN/llvm-nm"
[ -x "$LLD" ] || { echo "error: missing rust-lld ($LLD)" >&2; exit 1; }

# EXTRA_CFLAGS: see build-ball-tt.sh.
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fomit-frame-pointer -fno-pic -fno-builtin -nostdinc -funsigned-char
        -std=c99 -Wall -Wextra -Werror
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined ${EXTRA_CFLAGS:-})
INCS=(-I"$HERE" -I"$GEMTOP" -I"$MINT" -isystem "$RES")

echo "clang: $($CLANG --version | head -1)"
echo "abi  : LP64 (8-byte long/pointer, ELF64 AArch64)"
echo "src  : ball.c + ball-tex.c + plat-newtos.c"
echo "out  : $OUT"
echo

rm -f "$BUILD"/*.o
for s in ball.c ball-tex.c plat-newtos.c; do
    o="$BUILD/${s%.c}.o"
    if ! "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/$s" -o "$o" 2>"$BUILD/${s%.c}.err"; then
        echo "COMPILE FAILED ($s):"
        sed 's/^/    /' "$BUILD/${s%.c}.err"
        exit 1
    fi
    [ -s "$BUILD/${s%.c}.err" ] && { echo "warnings ($s):"; sed 's/^/    /' "$BUILD/${s%.c}.err"; }
    echo "  CC  $s"
done

"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1

rm -f "$OUT"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why.txt" \
    "$BUILD/crt0.o" "$BUILD/ball.o" "$BUILD/ball-tex.o" \
    "$BUILD/plat-newtos.o" \
    --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -ne 0 ]; then
    echo
    echo "LINK FAILED -- residual undefined symbols:"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link.err" | awk '{print $3}' | sort -u | sed 's/^/    /'
    echo "See $BUILD/why.txt for the archive pull chains."
    exit 1
fi

echo
echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
"$READOBJ" --file-headers "$OUT" | grep -iE 'Class:|DataEncoding:|Type:|Machine:|Entry:'
u=$("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)
echo "undefined symbols in the final image (goal 0): $u"
if [ "$u" -ne 0 ]; then
    echo "  residual undefined:"; "$NM" -u "$OUT" | sed 's/^/    /'
    exit 1
fi
