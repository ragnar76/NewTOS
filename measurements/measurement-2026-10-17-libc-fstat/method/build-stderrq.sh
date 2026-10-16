#!/usr/bin/env bash
# (LIBC-FSTAT: copied unchanged from measurement-2026-10-13-mem05-acceptance/method, with tests/ and crt0-before-libc-main.S.)
# build-stderrq.sh <outdir> -- STDERRQ.NAP: stderrp.c without the spawn (short output for the EmuCON screen run); CRT0=<file> selects another crt0
# PIE libc program, with the libc toolchain path of
# measurement-2026-10-13-crt0-main/method/build-s2-tests.sh (copied, trimmed):
#   FS11DEV.NAP  part A (device table, no input) + part B (keys + mintlib)
#   FS11A.NAP    part A only (-DFS11_NO_KEYS), for the product kernel runs
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-fs11.sh <outdir>}"
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
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
build() { # build <src> <OUT> [cflags...]
  local src="$1" out="$2"; shift 2
  "$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "${CRT0:-$LIBC/crt0.S}" -o "$W/crt0.o" || exit 1
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "$@" -c "$HERE/tests/$src.c" -o "$W/$src.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$W/crt0.o" "$W/$src.o" \
      --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$out" || exit 1
  echo "built $OUT/$out ($(wc -c < "$OUT/$out" | tr -d ' ') bytes)"
}
build stderrp STDERRQ.NAP -DSTDERRP_NOSPAWN
true
rm -rf "$W"
