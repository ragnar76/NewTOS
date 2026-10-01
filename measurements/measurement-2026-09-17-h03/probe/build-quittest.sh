#!/usr/bin/env bash
# build-quittest.sh MODE OUT.NAP -- same toolchain/flags as sdk/apps/gemtest/build-gemtest.sh
set -uo pipefail
MODE="$1"; OUT="$2"
SDK=/Volumes/Daten/Development/PiRusTOS/sdk
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$HERE/quittest.c"
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
BUILD="$HERE/build-m$MODE"; mkdir -p "$BUILD"
MINT="$LIBC/mintlib/include"
CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os
        -fomit-frame-pointer -fno-pic -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined -DMODE=$MODE)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES")
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$SRC" -o "$BUILD/quittest.o" || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 --build-id=none \
    "$BUILD/crt0.o" "$BUILD/quittest.o" \
    --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT" || exit 1
echo "built $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
