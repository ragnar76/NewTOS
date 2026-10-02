#!/usr/bin/env bash
#
# build-variants.sh <outdir>
#
# The three DOOM.NAP builds this measurement ran, each compiled by the
# delivered sdk/apps/build-doom.sh (same sources, same flags) into
# <outdir>/<variant>/, and then LINKED HERE with method/diag_system.o.
#
# Why the extra object: build-doom.sh's own link stops on a known libc gap
# (libc.a's system() needs spawnvp/vfork/wait4 pieces that do not build for
# LP64 -- sdk/libc/expected-failures.txt). doomgeneric calls system() only
# from ZenityErrorBox, which I_Error skips while stdout is a tty; isatty()
# is 1 for handles 0..2 on NewTOS (sdk/libc/newtos-arch/isatty.c), so the
# call is never made here. diag_system.c refuses loudly if it ever is. It is
# a MEASUREMENT device for this directory only, not part of the port.
#
#   play   DOOM_STATS=0, -mb 2              -- the delivered configuration
#   stats  DOOM_STATS=5, -mb 2              -- attract loop + cost probe
#   tdemo  DOOM_STATS=5, -mb 2 -timedemo demo1
set -euo pipefail
OUTDIR="${1:?usage: build-variants.sh <outdir>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"
SYSROOT="$(rustc --print sysroot)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
RES="$(clang -print-resource-dir)/include"
mkdir -p "$OUTDIR"
clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fno-pic \
    -fno-builtin -nostdinc -I"$SDK/libc/mintlib/include" -isystem "$RES" \
    -c "$HERE/diag_system.c" -o "$OUTDIR/diag_system.o"
build() {
    local v="$1" stats="$2" args="$3"
    BUILD="$OUTDIR/$v" DOOM_STATS="$stats" DOOM_ARGS="$args" \
        "$SDK/apps/build-doom.sh" > "$OUTDIR/$v.build.log" 2>&1 || true
    grep -q "compiled OK=81  FAILED=0" "$OUTDIR/$v.build.log" \
        || { echo "compile failed ($v), see $OUTDIR/$v.build.log"; exit 1; }
    "$BIN/rust-lld" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
        --build-id=none "$OUTDIR/$v/crt0.o" "$OUTDIR/$v"/obj/*.o "$OUTDIR/diag_system.o" \
        --start-group "$SDK/gemlib/gemlib.a" "$SDK/libm/libm.a" "$SDK/libc/libc.a" --end-group \
        -o "$OUTDIR/$v/DOOM.NAP"
    echo "$v: $(wc -c < "$OUTDIR/$v/DOOM.NAP" | tr -d ' ') bytes, _end=0x$("$BIN/llvm-nm" "$OUTDIR/$v/DOOM.NAP" | awk '$3=="_end"{print $1}'), undefined=$("$BIN/llvm-nm" -u "$OUTDIR/$v/DOOM.NAP" | grep -c . || true), sha256=$(shasum -a 256 "$OUTDIR/$v/DOOM.NAP" | cut -c1-16)"
}
build play  0 '"-mb","2"'
build stats 5 '"-mb","2"'
build tdemo 5 '"-mb","2","-timedemo","demo1"'
