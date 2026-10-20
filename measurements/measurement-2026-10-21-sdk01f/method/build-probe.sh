#!/usr/bin/env bash
# build-probe.sh <libc.a> <out.NCP> -- method/statt.c linked against the given libc.a (crt0 + statt.o + libc.a,
# link line of measurement-2026-10-20-sdk01a/method/build-probes.sh without gemlib, which statt.c does not use).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
LIBCA="${1:?libc.a}"; OUT="${2:?out}"
SDK="$WS/sdk"; LIBC="$SDK/libc"; MINT="$LIBC/mintlib/include"; CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
. "$SDK/toolchain.sh"; LLD="$(newtos_rust_bindir)/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__ -Wno-macro-redefined)
W="$(mktemp -d "$S/w.XXXX")"
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o"
"$CLANG" "${CFLAGS[@]}" -I"$MINT" -isystem "$RES" -I"$HERE" -c "$HERE/statt.c" -o "$W/p.o"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
  "$W/crt0.o" "$W/p.o" "$LIBCA" -o "$OUT"
rm -rf "$W"; shasum -a 256 "$OUT"
