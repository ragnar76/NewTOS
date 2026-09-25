#!/usr/bin/env bash
#
# build-wheel.sh <outdir>
#
# Compile method/wheeltst.c into WHEELT.NAP, a native NewTOS/AArch64 LP64 GEM
# application.
#
# The flag/include/link recipe is copied verbatim from
# measurement-2026-09-25-port05/method/build-pex.sh, which copied it from
# sdk/apps/chaintest/build-chaintest.sh -- so the program goes through EXACTLY
# the toolchain, headers, crt0 and ABI that TeraDesk and QED go through.
#
# Nothing is written into the shared tree except the two library archives the
# SDK builds anyway (sdk/gemlib/gemlib.a, sdk/libc/libc.a, unchanged inputs =
# no rebuild); the .o/.NAP land in <outdir>.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
SDK="$WS/sdk"
OUTDIR="${1:?usage: build-wheel.sh <outdir>}"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUTDIR"

. "$SDK/lib-stale.sh"
lib_ensure gemlib || exit 1
MINT="$LIBC/mintlib/include"

CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
NM="$BIN/llvm-nm"

CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os
        -fomit-frame-pointer -fno-pic -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES")

"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0.S" -o "$OUTDIR/crt0.o" || exit 1

name=wheeltst
if ! "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/$name.c" \
        -o "$OUTDIR/$name.o" 2>"$OUTDIR/$name.err"; then
    echo "COMPILE FAILED:"; sed 's/^/    /' "$OUTDIR/$name.err"; exit 1
fi
[ -s "$OUTDIR/$name.err" ] && { echo "warnings:"; sed 's/^/    /' "$OUTDIR/$name.err"; }

rm -f "$OUTDIR/WHEELT.NAP"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none \
    "$OUTDIR/crt0.o" "$OUTDIR/$name.o" \
    --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group \
    -o "$OUTDIR/WHEELT.NAP" 2>"$OUTDIR/$name.link.err" || {
        echo "LINK FAILED:"; sed 's/^/    /' "$OUTDIR/$name.link.err"; exit 1; }
u=$("$NM" -u "$OUTDIR/WHEELT.NAP" 2>/dev/null | grep -c . || true)
echo "built $OUTDIR/WHEELT.NAP ($(wc -c < "$OUTDIR/WHEELT.NAP" | tr -d ' ') bytes, undefined=$u)"
