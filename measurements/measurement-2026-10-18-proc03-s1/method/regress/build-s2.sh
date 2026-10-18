#!/usr/bin/env bash
# build-s2.sh <outdir> -- the ARCH03 stage-2 libc probes S2ALL/S2ALLS run (PFLAGS1/2, P34PAR/KID, ENVPAR/KID, P567(S),
# TSRPAR(S)/TSRRES/TSRUSE, OWNPAR/KID, VFORKT, S2ALL, S2ALLS), built through the libc toolchain path of the ported apps.
# Copied (trimmed to these programs) from measurements/measurement-2026-10-13-libc-tty-acceptance/method/build-s2-tests.sh;
# tests/ = the same sources, copied unchanged (envkidm.c from measurement-2026-10-13-mem05-acceptance/method/tests).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../../.." && pwd)"
SDK="$WS/sdk"
OUT="${1:?usage: build-s2.sh <outdir>}"
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
  echo "built $out"
}
for n in envpar p34par p34kid; do build $n "$(echo $n | tr '[:lower:]' '[:upper:]').NAP"; done
# ENVKID from tests/envkidm.c (crt0-main expectations; the S2ALLM variant of the 2026-10-13 runs -- the plain
# envkid.c is stale since crt0-main: "ARGV= arrived" FAIL, measurement-2026-10-13-libc-tty-acceptance tta-*-s2all).
build envkidm ENVKID.NAP
build pflags PFLAGS1.NCP 0x10000004 -DWANT_FLAGS=0x10000004UL
build pflags PFLAGS2.NCP 0x10000004 -DWANT_FLAGS=0x7UL
(cd "$WS" && cargo xtask prgflags "$OUT/PFLAGS2.NCP" 0x7 >/dev/null 2>&1 && cargo xtask prgflags "$OUT/PFLAGS2.NCP" >/dev/null 2>&1) || exit 1
for n in p567 tsrpar tsrres tsruse ownpar ownkid; do build $n "$(echo $n | tr '[:lower:]' '[:upper:]').NAP"; done
build vforkt VFORKT.NAP
build s2all S2ALL.NAP
build p567 P567S.NAP "" -DP567_STRICT
build tsrpar TSRPARS.NAP "" -DTSRPAR_STRICT
build s2all S2ALLS.NAP "" -DS2ALL_STRICT
rm -rf "$W"
