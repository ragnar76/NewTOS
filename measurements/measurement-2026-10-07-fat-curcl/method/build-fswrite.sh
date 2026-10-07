#!/usr/bin/env bash
#
# build-fswrite.sh <OUT.NCP> -- build FSWRITE.NCP (method/fswrite.c), the
# headless GEMDOS write-path check of TASK/FS08 part 2, against
# sdk/libc/libc.a. Flags and link line copied from
# sdk/examples/fsbench/build-fsbench.sh (object dir in $TMPDIR).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK="$(cd "$HERE/../../../sdk" && pwd)"
OUT="${1:?OUT.NCP}"
OBJ="$(mktemp -d "${TMPDIR:?set TMPDIR to the scratch dir}/fswrite.XXXXXX")"
trap 'rm -rf "$OBJ"' EXIT
. "$SDK/lib-stale.sh"
lib_ensure libc || exit 1
MINT="$SDK/libc/mintlib/include"
CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__
        -Wno-macro-redefined -Wall -Wextra)
"$CLANG" "${CFLAGS[@]}" -I"$MINT" -isystem "$RES" -c "$HERE/fswrite.c" -o "$OBJ/fswrite.o"
"$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc \
    -I"$SDK/include" -I"$MINT" -c "$SDK/libc/crt0.S" -o "$OBJ/crt0.o"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none "$OBJ/crt0.o" "$OBJ/fswrite.o" "$SDK/libc/libc.a" -o "$OUT"
echo "FSWRITE.NCP -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
echo "  undefined symbols (goal 0): $("$BIN/llvm-nm" -u "$OUT" 2>/dev/null | grep -c . || true)"
