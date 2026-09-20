#!/usr/bin/env bash
# build-probe.sh SRC.c OUT.NAP -- same flags/libs as sdk/apps/chaintest/build-chaintest.sh,
# objects in scratch; reads the project's sdk read-only.
set -uo pipefail
SRC="$1"; OUT="$2"
SDK=/Volumes/Daten/Development/PiRusTOS/sdk
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"; MINT="$LIBC/mintlib/include"
BUILD="$(dirname "$OUT")/build-$(basename "$SRC" .c)"; mkdir -p "$BUILD"
CLANG=clang; RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot)"; BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os -fomit-frame-pointer -fno-pic -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES")
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$SRC" -o "$BUILD/app.o" || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 --build-id=none \
  "$BUILD/crt0.o" "$BUILD/app.o" --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT" || exit 1
echo "built $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
