#!/usr/bin/env bash
# build-fstatp.sh <outdir> [old-libc.a] -- LIBC-FSTAT QEMU probe tests/fstatp.c as an LP64 PIE libc program
# (build() copied from build-mem05b.sh = measurement-2026-10-13-mem05-acceptance/method, plus -Wall -Wextra -Wno-unused-function):
#   FSTATP.NAP      against the CURRENT sdk/libc/libc.a (with the LIBC-FSTAT do_fstat.c)
#   FSTATP-OLD.NAP  (if old-libc.a is given: the libc.a before LIBC-FSTAT) -- red control; put on the card as C:\FSTATP.NAP
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-fstatp.sh <outdir> [old-libc.a]}"; OLD="${2:-}"
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
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined -Wall -Wextra -Wno-unused-function)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$HERE/tests")
W="$(mktemp -d "$OUT/w.XXXX")"
build() { # build <src> <OUT> [prgflags] [cflags...]   (LIBCA=<archive> overrides libc.a)
  local src="$1" out="$2" pf="${3:-}"; shift 2; [ $# -gt 0 ] && shift
  local d=(); [ -n "$pf" ] && d=("-DNEWTOS_PRGFLAGS=$pf")
  "$CLANG" "${CFLAGS[@]}" ${d[@]+"${d[@]}"} -I"$SDK/include" -I"$MINT" -c "${CRT0:-$LIBC/crt0.S}" -o "$W/crt0.o" || exit 1
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "$@" -c "$HERE/tests/$src.c" -o "$W/$src.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$W/crt0.o" "$W/$src.o" \
      --start-group "$GEM/gemlib.a" "${LIBCA:-$LIBC/libc.a}" --end-group -o "$OUT/$out" || exit 1
  echo "built $OUT/$out ($(wc -c < "$OUT/$out" | tr -d ' ') bytes)${pf:+ PRGFLAGS=$pf}"
}
build fstatp FSTATP.NAP
[ -n "$OLD" ] && LIBCA="$OLD" build fstatp FSTATP-OLD.NAP
rm -rf "$W"
shasum -a 256 "$LIBC/libc.a" ${OLD:+"$OLD"} | sed "s|$WS/||"
