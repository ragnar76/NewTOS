#!/usr/bin/env bash
#
# build-scrninfo-newtos.sh -- build SCRNINFO.NAP, the NewTOS/AArch64 build of
# the EdDI / vq_scrninfo probe (core/scrninfo.c; spec
# docs/api/vdi/eddi-vq_scrninfo.md).
#
# Same portable core as the m68k SCRNINFO.TTP (core/gem.c, core/scrninfo.c);
# only the backends differ: abi/newtos_svc.c instead of abi/m68k_trap.c for
# the VDI/AES traps, abi/newtos_scrninfo.c instead of abi/m68k_scrninfo.c for
# the cookie jar, the (refused) EdDI dispatcher call and the clock. Link path
# as build-dumpvdi-newtos.sh: clang -target aarch64-unknown-none-elf, the
# libc-aware crt0, mintlib's libc.a, sdk/newtos.ld, no gemlib.
#
# It is a .NAP (native GEM application): it calls appl_init / graf_handle /
# v_opnvwk, so it is started by the desktop or by a #Z autorun line in
# NEWDESK.INF, not from C:\AUTO. Without an argument it writes SCRNINFO.TXT
# into the current folder.
#
# Usage:  tosdump/build-scrninfo-newtos.sh [output.NAP]
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"        # .../tosdump
ROOT="$(cd "$HERE/.." && pwd)"
SDK="$ROOT/sdk"
LIBC="$SDK/libc"
BUILD="$HERE/build"
OBJ="$BUILD/obj-scrninfo-newtos"
OUT="${1:-$BUILD/SCRNINFO.NAP}"
mkdir -p "$OBJ"

# --- libc ---------------------------------------------------------------------
if [ ! -f "$LIBC/libc.a" ]; then
  echo "libc.a absent -- running build-libc.sh first…"
  "$LIBC/build-libc.sh" >/dev/null 2>&1 || { echo "error: build-libc.sh failed" >&2; exit 1; }
fi
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ] || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }

# --- toolchain ----------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
READOBJ="$BIN/llvm-readobj"
NM="$BIN/llvm-nm"
[ -x "$LLD" ] || { echo "error: missing rust-lld ($LLD)" >&2; exit 1; }

# The header's build id field is exactly 4 bytes; git widens --short when 4
# hex digits are not unique, so cut it to what actually gets stored.
BUILD_ID="$(cd "$ROOT" && git rev-parse HEAD 2>/dev/null | cut -c1-4)"
[ -n "$BUILD_ID" ] || BUILD_ID='----'

# Same flags as the other NewTOS tosdump builds. TD_BIG_ENDIAN_TARGET only
# matters to the record writer, which the probe does not link; the probe's
# "BEwords" lines assemble big-endian words from the bytes explicitly.
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc -D__NEWTOS__
        -std=c99 -Wall -Wextra -Werror
        -DTD_BIG_ENDIAN_TARGET=0 -DTD_BUILD_ID="\"$BUILD_ID\"")
INCS=(-I"$HERE/core" -I"$MINT" -isystem "$RES")

echo "clang : $($CLANG --version | head -1)"
echo "lld   : $LLD"
echo "build : $BUILD_ID"
echo

SRC=(core/gem.c core/scrninfo.c abi/newtos_scrninfo.c abi/newtos_svc.c)

rm -f "$OBJ"/*.o
for s in "${SRC[@]}"; do
    o="$OBJ/$(echo "${s%.c}" | tr '/' '__').o"
    if ! "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/$s" -o "$o"; then
        echo "error: $s failed to compile" >&2
        exit 1
    fi
    echo "  CC  $s"
done

"$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 \
    -fPIE -fno-builtin -nostdinc -I"$SDK/include" -I"$MINT" -isystem "$RES" \
    -c "$LIBC/crt0.S" -o "$BUILD/crt0-scrninfo-newtos.o" || exit 1

rm -f "$OUT"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why-scrninfo-newtos.txt" \
    "$BUILD/crt0-scrninfo-newtos.o" "$OBJ"/*.o "$LIBC/libc.a" \
    -o "$OUT" 2>"$BUILD/link-scrninfo-newtos.err"
lr=$?

if [ $lr -ne 0 ]; then
    echo
    echo "LINK FAILED -- residual undefined symbols:"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link-scrninfo-newtos.err" \
        | awk '{print $3}' | sort -u | sed 's/^/    /'
    exit 1
fi

echo
echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
"$READOBJ" --file-headers "$OUT" | grep -iE 'Class:|DataEncoding:|Type:|Machine:|Entry:'
u=$("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)
echo "undefined symbols in the final image (goal 0): $u"
[ "$u" -eq 0 ] || exit 1
