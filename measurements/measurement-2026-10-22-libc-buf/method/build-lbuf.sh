#!/usr/bin/env bash
# build-lbuf.sh <outdir> [libc.a] -- LBUF.NCP from lbuf.c through the libc/gemlib path of the ported apps
# (adapted from measurement-2026-10-19-ticket-review/method/build-libcbuf.sh, + -Wall -Wextra, libc.a selectable for the red control).
# Writes only <outdir>.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"; OUT="${1:?outdir}"; LIBCA="${2:-$SDK/libc/libc.a}"
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
[ -f "$GEM/gemlib.a" ] && [ -f "$LIBCA" ] || { echo "gemlib.a/libc.a missing (build them first)"; exit 1; }
MINT="$LIBC/mintlib/include"; CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"; LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined -Wall -Wextra)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$HERE")
W="$(mktemp -d "$OUT/w.XXXX")"
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/lbuf.c" -o "$W/lbuf.o" || exit 1
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
  "$W/crt0.o" "$W/lbuf.o" --start-group "$GEM/gemlib.a" "$LIBCA" --end-group -o "$OUT/LBUF.NCP" || exit 1
rm -rf "$W"
shasum -a 256 "$OUT/LBUF.NCP" "$LIBCA" | sed "s|$WS/||"
