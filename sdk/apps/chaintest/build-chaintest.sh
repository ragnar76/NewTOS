#!/usr/bin/env bash
#
# build-chaintest.sh -- compile CHAINTEST (the NewTOS AES/VDI diagnostic, the GEM
# counterpart to the console APITEST) into a native NewTOS/AArch64 **LP64** GEM
# application (CHAINTST.NAP, per ADR-018) against the ported mintlib (libc.a) +
# gemlib (gemlib.a) -- the SAME toolchain, headers and ABI that build-teradesk.sh
# uses. This is deliberate: CHAINTEST must reproduce TeraDesk's exact LP64 gemlib
# path so any gemlib-glue or kernel-decode bug bites identically.
#
# Modelled on sdk/apps/build-teradesk.sh, minus the shadow-tree/overlay machinery
# (CHAINTEST is a single vendored-free source file, so there is nothing to overlay):
#   CFLAGS = -Os -fomit-frame-pointer -D__NEWTOS__ -D_TOS_ONLY + the shared
#            freestanding/no-libc set (identical to build-teradesk.sh).
#   INCS   = gemlib top (gem.h/mt_gem.h/portaes.h/portvdi.h) + mintlib + clang RES.
#   link   = crt0 + chaintest.o + gemlib.a + libc.a via sdk/newtos.ld -> CHAINTST.NAP.
#
#   objs   : sdk/apps/chaintest/build/
#   output : sdk/apps/chaintest/CHAINTST.NAP  (ELF64 AArch64)
#
# Usage:  sdk/apps/chaintest/build-chaintest.sh [output.NAP]
#   output.NAP  defaults to sdk/apps/chaintest/CHAINTST.NAP
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # sdk/apps/chaintest
SDK="$(cd "$HERE/../.." && pwd)"                          # sdk
SRC="$HERE/chaintest.c"
OUT="${1:-$HERE/CHAINTST.NAP}"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
GEMTOP="$GEM/gemlib-0.44.0"
BUILD="$HERE/build"
mkdir -p "$BUILD"

[ -f "$SRC" ] || { echo "error: $SRC missing" >&2; exit 1; }

# --- ensure the two ported libraries are current ------------------------------
# gemlib -> libc, the same chain build-teradesk.sh links. The chain and each
# library's real input set live once in sdk/lib-stale.sh (TASK/T03);
# `lib_ensure gemlib` walks libc -> gemlib in that order and does not swallow
# what the library builds report (TASK/T02). Before TASK/T06 this script only
# built a library whose ARCHIVE was absent, so a stale one was reused in
# silence -- the pre-T03 pattern lib-stale.sh's header describes.
. "$SDK/lib-stale.sh"
lib_ensure gemlib || exit 1
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ]   || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }
[ -d "$GEMTOP" ] || { echo "error: gemlib headers missing ($GEMTOP); run build-gemlib.sh" >&2; exit 1; }

# --- toolchain ---------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
. "$SDK/toolchain.sh"   # LLVM tools: PATH / LLVM packages first, pinned Rust sysroot as fallback (SDK01 I, K)
AR="${AR:-$(newtos_tool llvm-ar)}"
LLD="${LLD:-$(newtos_tool ld.lld)}"
READOBJ="$(newtos_tool llvm-readobj)"
NM="$(newtos_tool llvm-nm)"
OBJDUMP="$(newtos_tool llvm-objdump)"
for t in "$AR" "$LLD"; do
    [ -x "$t" ] || { echo "error: missing LLVM tool: $t" >&2; exit 1; }
done

# --- flags (IDENTICAL to build-teradesk.sh) ----------------------------------
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os
        -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
# Include order: gemlib top (gem.h/mt_gem.h/portaes.h/portvdi.h/tos.h), mintlib
# include (mint/osbind.h), clang headers last.
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES")

echo "clang: $($CLANG --version | head -1)"
echo "abi  : LP64 (8-byte long/pointer, ELF64 AArch64)"
echo "libs : gemlib.a + libc.a"
echo "src  : $SRC"
echo "out  : $OUT"
echo

# --- compile the diagnostic --------------------------------------------------
if ! "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$SRC" -o "$BUILD/chaintest.o" 2>"$BUILD/chaintest.err"; then
    echo "COMPILE FAILED:"
    sed 's/^/    /' "$BUILD/chaintest.err"
    exit 1
fi
[ -s "$BUILD/chaintest.err" ] && { echo "compile warnings:"; sed 's/^/    /' "$BUILD/chaintest.err"; }
echo "compiled chaintest.c OK"

# --- CRT ---------------------------------------------------------------------
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1

# --- link (crt0 + chaintest.o + gemlib.a + libc.a) -----------------------------
# gemlib -> libc dependency order; wrap in a group so any residual
# back-reference between the two archives still resolves.
rm -f "$OUT"
newtos_lld -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why.txt" \
    "$BUILD/crt0.o" "$BUILD/chaintest.o" \
    --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    "$READOBJ" --file-headers "$OUT" | grep -iE 'Class:|DataEncoding:|Type:|Machine:|Entry:'
    echo
    u=$("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)
    echo "undefined symbols in the final image (goal 0): $u"
    if [ "$u" -ne 0 ]; then
        echo "  residual undefined:"; "$NM" -u "$OUT" | sed 's/^/    /'
    fi
    echo
    echo "GEM trap sites (svc #2) reachable from the linked program:"
    "$OBJDUMP" -d "$OUT" | grep -iE '<aes>:|<vdi>:|svc[[:space:]]+#0x2' | head | sed 's/^/    /'
else
    echo
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link.err" | awk '{print $3}' | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/why.txt for the archive pull chains that reference them."
    exit 1
fi
