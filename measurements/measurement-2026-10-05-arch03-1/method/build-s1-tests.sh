#!/usr/bin/env bash
# build-s1-tests.sh <outdir> -- build the ARCH03 stage-1 probes (method/tests)
# as LP64 PIE programs through EXACTLY the libc toolchain path the ported apps
# take (flags/link line of sdk/apps/chaintest/build-chaintest.sh after 0d):
# ENVPAR.NAP + ENVKID.NAP (MEM03), P34PAR.NAP + P34KID.NAP (Pexec 3+4).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-s1-tests.sh <outdir>}"
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUT"
. "$SDK/lib-stale.sh"
lib_ensure gemlib || exit 1
MINT="$LIBC/mintlib/include"
CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os
        -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$HERE/tests")
W="$(mktemp -d "$OUT/w.XXXX")"
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
for n in envpar envkid p34par p34kid; do
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/tests/$n.c" -o "$W/$n.o" || exit 1
  up=$(echo "$n" | tr '[:lower:]' '[:upper:]')
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$W/crt0.o" "$W/$n.o" \
      --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$up.NAP" || exit 1
  echo "built $OUT/$up.NAP ($(wc -c < "$OUT/$up.NAP" | tr -d ' ') bytes)"
done
rm -rf "$W"
