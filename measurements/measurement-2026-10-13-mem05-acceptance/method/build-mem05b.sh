#!/usr/bin/env bash
# build-mem05b.sh <outdir> [old-libc.a] -- MEM05 part B probes (method/tests), built
# with build() of build-mem05.sh (part A, = measurement-2026-10-13-libc-tty-acceptance/
# method/build-s2-tests.sh): against the CURRENT sdk/libc/libc.a (part B: MAXHUNK 16 MiB)
#   MDPROBE.NAP  part A's tests/mdprobe.c unchanged (malloc(32) x 3 000 000, probes at
#                start/end only) -- the before/after pair with part A's MDPROBE.NAP
#   MDSTEP.NAP   tests/mdstep.c: MDs + mintlib hunks per step up to 10 000 000
#   MDTIME.NAP   tests/mdstep.c -DMDSTEP_NOPROBE: the same series, timing only
#   HELLOMEM.NAP tests/hellomem.c (tag "new")
#   MDACC.NAC    tests/mdacc.c, accessory (design QEMU test 15)
# and, if old-libc.a is given (the libc.a before part B, sha256 eb9cd4f8...):
#   HELLOOLD.NAP tests/hellomem.c (tag "old")
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-mem05b.sh <outdir> [old-libc.a]}"; OLD="${2:-}"
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
build mdprobe MDPROBE.NAP
build mdstep MDSTEP.NAP
build mdstep MDTIME.NAP "" -DMDSTEP_NOPROBE
build hellomem HELLOMEM.NAP "" '-DHELLO_TAG="new"'
build mdacc MDACC.NAC
[ -n "$OLD" ] && LIBCA="$OLD" build hellomem HELLOOLD.NAP "" '-DHELLO_TAG="old"'
rm -rf "$W"
shasum -a 256 "$LIBC/libc.a" ${OLD:+"$OLD"} | sed "s|$WS/||"
