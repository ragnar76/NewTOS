#!/usr/bin/env bash
#
# build-port09.sh <outdir>
#
# Compile the PORT09 probe pair (method/envpar.c + method/envkid.c) into
# native NewTOS/AArch64 LP64 programs, ENVPAR.NAP and ENVKID.NAP (recipe of measurement-2026-09-25-port05/method/build-port09.sh).
#
# The flag/include/link recipe is copied verbatim from
# sdk/apps/chaintest/build-chaintest.sh so the two programs go through EXACTLY
# the toolchain, headers, crt0 and ABI that TeraDesk and QED go through -- the
# point of the measurement is the kernel's launch path, so nothing about the
# programs may differ from a real ported app.
#
# Nothing is written into the shared tree except the two library archives the
# SDK builds anyway (sdk/gemlib/gemlib.a, sdk/libc/libc.a, unchanged inputs =
# no rebuild); the .o/.NAP land in <outdir>.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
OUTDIR="${1:?usage: build-port09.sh <outdir>}"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUTDIR"

. "$SDK/lib-stale.sh"
lib_ensure gemlib || exit 1
MINT="$LIBC/mintlib/include"

CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
NM="$BIN/llvm-nm"

CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os
        -fomit-frame-pointer -fno-pic -fno-builtin -nostdinc -funsigned-char
        -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES")

"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0.S" -o "$OUTDIR/crt0.o" || exit 1

# envpar = the parent, launching C:\ENVKID.NAP; envpara = the same parent,
# launching C:\ARGVTEST.NAP (the existing SDK example) -- what the libc
# startup makes of the same tail + environment.
for spec in "envpar:envpar.c:" "envkid:envkid.c:" \
            "envpara:envpar.c:-DCHILD_PATH=\"C:\\\\ARGVTEST.NAP\""; do
    name="${spec%%:*}"; rest="${spec#*:}"; src="${rest%%:*}"; extra="${rest#*:}"
    IFS='|' read -r -a xflags <<< "$extra"
    if ! "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" ${extra:+"${xflags[@]}"} -c "$HERE/$src" \
            -o "$OUTDIR/$name.o" 2>"$OUTDIR/$name.err"; then
        echo "COMPILE FAILED ($name):"; sed 's/^/    /' "$OUTDIR/$name.err"; exit 1
    fi
    [ -s "$OUTDIR/$name.err" ] && { echo "warnings ($name):"; sed 's/^/    /' "$OUTDIR/$name.err"; }
    up=$(echo "$name" | tr '[:lower:]' '[:upper:]')
    rm -f "$OUTDIR/$up.NAP"
    "$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
        --build-id=none \
        "$OUTDIR/crt0.o" "$OUTDIR/$name.o" \
        --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group \
        -o "$OUTDIR/$up.NAP" 2>"$OUTDIR/$name.link.err" || {
            echo "LINK FAILED ($name):"; sed 's/^/    /' "$OUTDIR/$name.link.err"; exit 1; }
    u=$("$NM" -u "$OUTDIR/$up.NAP" 2>/dev/null | grep -c . || true)
    echo "built $OUTDIR/$up.NAP ($(wc -c < "$OUTDIR/$up.NAP" | tr -d ' ') bytes, undefined=$u)"
done
