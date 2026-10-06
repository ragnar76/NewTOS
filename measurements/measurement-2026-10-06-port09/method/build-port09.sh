#!/usr/bin/env bash
# build-port09.sh <outdir> -- the PORT09 probes as LP64 PIE programs through
# EXACTLY the libc toolchain path the ported apps take (recipe of
# build-s2-tests.sh in this directory, i.e. sdk/apps/chaintest/build-chaintest.sh):
#   P9EPAR.NAP  = port09-envpar.c (measurement-2026-10-02-port09/method/envpar.c
#                 unchanged) launching C:\P9EKID.NAP: Pexec(0, kid,
#                 tail[0]=127 + 125 chars, env with ARGV= + 14 arguments)
#   P9EKID.NAP  = port09-envkid.c (ibid. envkid.c unchanged): prints p_env and
#                 walks the block -- the "can a child read p_env" check
#   P9APAR.NAP  = the same parent launching C:\ARGVTEST.NAP (what the libc
#                 start-up makes of that tail + environment)
#   ARGVTEST.NAP = sdk/examples/argvtest.c (libc start-up -> argc/argv)
#   P9PAR.NAP / P9KID.NAP = tests/p9par.c / tests/p9kid.c: the case suite
#                 (ARGV, ARGV=NULL:, no ARGV, empty env, inherited env)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-port09.sh <outdir>}"
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
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
build() { # build <src path> <OUT> [cflags...]
  local src="$1" out="$2"; shift 2
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "$@" -c "$src" -o "$W/$out.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$W/crt0.o" "$W/$out.o" \
      --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$out" || exit 1
  echo "built $OUT/$out ($(wc -c < "$OUT/$out" | tr -d ' ') bytes)"
}
build "$HERE/port09-envpar.c" P9EPAR.NAP '-DCHILD_PATH="C:\\P9EKID.NAP"'
build "$HERE/port09-envkid.c" P9EKID.NAP
build "$HERE/port09-envpar.c" P9APAR.NAP '-DCHILD_PATH="C:\\ARGVTEST.NAP"'
build "$SDK/examples/argvtest.c" ARGVTEST.NAP
[ -f "$HERE/tests/p9par.c" ] && build "$HERE/tests/p9par.c" P9PAR.NAP
[ -f "$HERE/tests/p9kid.c" ] && build "$HERE/tests/p9kid.c" P9KID.NAP
rm -rf "$W"
