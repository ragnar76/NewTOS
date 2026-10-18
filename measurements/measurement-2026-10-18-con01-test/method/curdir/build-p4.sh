#!/usr/bin/env bash
# build-p4.sh <outdir> -- build the PROC04 point-2 probe roles from p4probe.c through the libc/gemlib path
# of the ported apps (adapted from measurements/measurement-2026-10-17-proc03/method/build-p3.sh). Writes only <outdir>.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../../.." && pwd)"
SDK="$WS/sdk"; OUT="${1:?outdir}"
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
[ -f "$GEM/gemlib.a" ] && [ -f "$LIBC/libc.a" ] || { echo "gemlib.a/libc.a missing (build them first)"; exit 1; }
MINT="$LIBC/mintlib/include"; CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"; LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$HERE")
W="$(mktemp -d "$OUT/w.XXXX")"
build() { local out="$1"; shift
  "$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "$@" -c "$HERE/p4probe.c" -o "$W/p4.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
    "$W/crt0.o" "$W/p4.o" --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$out" || exit 1
  echo "built $out"; }
build P4AUTO.NCP -DP4_AUTO '-DP4_TAG="AUTO"'
build P4Z.NAP -DP4_GEM '-DP4_TAG="Z"'
build P4ACC.NAC -DP4_ACC '-DP4_TAG="ACC"'
build P4E.NCP -DP4_EMU '-DP4_TAG="EMU"'
cp "$OUT/P4Z.NAP" "$OUT/P4Z2.NAP"; cp "$OUT/P4Z.NAP" "$OUT/P4KZ.NAP"; cp "$OUT/P4AUTO.NCP" "$OUT/P4KA.NCP"
printf 'inner\r\n' > "$OUT/P4INNER.TXT"
rm -rf "$W"
