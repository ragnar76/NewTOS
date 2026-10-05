#!/usr/bin/env bash
# build-s2-tests.sh <outdir> -- build the ARCH03 probes (method/tests) as LP64
# PIE programs through EXACTLY the libc toolchain path the ported apps take
# (flags/link line of sdk/apps/chaintest/build-chaintest.sh): the stage-1
# probes ENVPAR/ENVKID (MEM03) and P34PAR/P34KID (Pexec 3+4), plus the
# stage-2 probes. Per program: "<source> <OUT> [PRGFLAGS] [extra -D...]".
# The PRGFLAGS value goes into crt0's note (-DNEWTOS_PRGFLAGS, ARCH03 E8b).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-s2-tests.sh <outdir>}"
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
build() { # build <src> <OUT> [prgflags] [cflags...]
  local src="$1" out="$2" pf="${3:-}"; shift 2; [ $# -gt 0 ] && shift
  local d=(); [ -n "$pf" ] && d=("-DNEWTOS_PRGFLAGS=$pf")
  "$CLANG" "${CFLAGS[@]}" ${d[@]+"${d[@]}"} -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "$@" -c "$HERE/tests/$src.c" -o "$W/$src.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$W/crt0.o" "$W/$src.o" \
      --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$out" || exit 1
  echo "built $OUT/$out ($(wc -c < "$OUT/$out" | tr -d ' ') bytes)${pf:+ PRGFLAGS=$pf}"
}
for n in envpar envkid p34par p34kid; do build $n "$(echo $n | tr '[:lower:]' '[:upper:]').NAP"; done
# 2a (E8b): the note's value read back; the second copy is patched by the tool.
build pflags PFLAGS1.NCP 0x10000004 -DWANT_FLAGS=0x10000004UL
build pflags PFLAGS2.NCP 0x10000004 -DWANT_FLAGS=0x7UL
(cd "$WS" && cargo xtask prgflags "$OUT/PFLAGS2.NCP" 0x7 2>/dev/null && cargo xtask prgflags "$OUT/PFLAGS2.NCP" 2>/dev/null) || exit 1
(cd "$WS" && cargo xtask prgflags "$OUT/PFLAGS1.NCP" 2>/dev/null)
# 2b (Pexec 5/6/7), 2c (Ptermres), 2d (owners, E15)
for n in p567 tsrpar tsrres tsruse ownpar ownkid; do build $n "$(echo $n | tr '[:lower:]' '[:upper:]').NAP"; done
# 2e (mintlib vfork + system() POSIX branch)
build vforkt VFORKT.NAP
build s2all S2ALL.NAP
for t in ${S2_TESTS:-}; do build $t "$(echo $t | tr '[:lower:]' '[:upper:]').NAP"; done
rm -rf "$W"
