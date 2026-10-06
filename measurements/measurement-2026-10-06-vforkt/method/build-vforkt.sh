#!/usr/bin/env bash
# build-vforkt.sh <crt0.S> <out.NAP> -- build the post-E9 VFORKT probe
# (measurement-2026-10-06-post-e9/method/tests/vforkt.c, unchanged) against a
# chosen crt0, with exactly the flags/link line of that run's
# build-s2-tests.sh and the tree's current libc.a/gemlib.a (read-only).
set -euo pipefail
CRT0="${1:?crt0}"; OUT="${2:?out}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
T="$WS/measurements/measurement-2026-10-06-post-e9/method/tests"
SDK="$WS/sdk"; GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
MINT="$LIBC/mintlib/include"; CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
LLD="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os
        -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
W="$(mktemp -d "$(dirname "$OUT")/w.XXXX")"
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$CRT0" -o "$W/crt0.o"
"$CLANG" "${CFLAGS[@]}" -I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$T" -c "$T/vforkt.c" -o "$W/vforkt.o"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none "$W/crt0.o" "$W/vforkt.o" \
    --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT"
rm -rf "$W"
echo "built $OUT"
