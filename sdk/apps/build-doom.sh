#!/usr/bin/env bash
#
# build-doom.sh -- compile doomgeneric (id Software's DOOM, GPLv2, via
# https://github.com/ozkl/doomgeneric) into a native NewTOS/AArch64 GEM
# application (DOOM.NAP, ADR-018) against the ported mintlib (libc.a),
# gemlib (gemlib.a) and OpenLibm (libm.a). TASK/PORT06.
#
# SOURCES
#   Applications/doomgeneric/      the vendored doomgeneric tree, pinned at
#                                  commit dcb7a8dbc7a16ce3dda29382ac9aae9d77d21284,
#                                  UNTRACKED and NEVER EDITED (memory note
#                                  "port-target-app-unmodified"). This script
#                                  refuses to build if the tree is at another
#                                  commit or has a modified tracked file.
#   sdk/apps/doom/doomgeneric_gem.c  OUR platform layer -- the doomgeneric
#                                  contract DG_Init / DG_DrawFrame /
#                                  DG_SleepMs / DG_GetTicksMs / DG_GetKey /
#                                  DG_SetWindowTitle plus main(), written
#                                  against AES / VDI / BIOS / XBIOS / GEMDOS
#                                  only (no XBIOS screen-base call, no
#                                  framebuffer pointer). It takes the place of
#                                  doomgeneric_xlib.c in upstream's manifest.
#
# The source manifest is upstream's own: SRC_DOOM from doomgeneric/Makefile,
# with doomgeneric_xlib.o swapped for our platform file -- so a file upstream
# adds or drops is picked up without editing this script.
#
# CONFIGURATION (compile-time, the doomgeneric way)
#   -DCMAP256                  the engine keeps its 8-bit palette picture and
#                              hands it to the platform unconverted (PORT06
#                              §3a: the colour conversion happens once, at
#                              the end, in the platform layer)
#   -DDOOMGENERIC_RESX=320     doomgeneric's output buffer is exactly DOOM's
#   -DDOOMGENERIC_RESY=200     own 320x200 picture; any integer zoom is the
#                              platform's business (doomgeneric_gem.c)
#   byte order                 doomgeneric's own mechanism (i_swap.h tests
#                              __BYTE_ORDER__); nothing to set here, and the
#                              later m68k build gets SYS_BIG_ENDIAN the same way.
#
# Environment knobs:
#   DOOM_ARGS='"-mb","2"'      extra arguments the platform appends to argv
#                              when the program was started without them (a
#                              desktop / #Z start passes no command tail).
#                              Default below; the zone size is the reason, see
#                              doomgeneric_gem.c "MEMORY".
#   DOOM_STATS=<seconds>       compile in a periodic frame-time report on the
#                              console (0 = off, the default).
#   OUT=<path>                 output file (default sdk/apps/doom/build/DOOM.NAP)
#
# Usage:  sdk/apps/build-doom.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"        # .../sdk
WS="$(cd "$SDK/.." && pwd)"
PORT="$SDK/apps/doom"                                         # our platform layer
DG="${DOOMGENERIC:-$WS/Applications/doomgeneric}"             # vendored tree
DGSRC="$DG/doomgeneric"
PIN="dcb7a8dbc7a16ce3dda29382ac9aae9d77d21284"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
LIBM="$SDK/libm"
GEMTOP="$GEM/gemlib-0.44.0"
LIBMTOP="$LIBM/openlibm-0.8.8"
BUILD="${BUILD:-$PORT/build}"
OBJ="$BUILD/obj"
OUT="${OUT:-$BUILD/DOOM.NAP}"
DOOM_ARGS="${DOOM_ARGS:-\"-mb\",\"2\"}"
DOOM_STATS="${DOOM_STATS:-0}"
mkdir -p "$OBJ"

[ -d "$DGSRC" ] || { echo "error: doomgeneric source missing ($DGSRC)" >&2; exit 1; }
[ -f "$PORT/doomgeneric_gem.c" ] || { echo "error: platform layer missing ($PORT/doomgeneric_gem.c)" >&2; exit 1; }

# --- the vendored tree must be the pinned, unmodified upstream ---------------
# Read-only git queries only. A tree without .git (e.g. an unpacked tarball)
# cannot be checked this way; say so instead of pretending.
if [ -d "$DG/.git" ] && command -v git >/dev/null 2>&1; then
    head_rev="$(git -C "$DG" rev-parse HEAD 2>/dev/null || echo unknown)"
    if [ "$head_rev" != "$PIN" ]; then
        echo "STOP: doomgeneric is at $head_rev, the port is pinned to $PIN." >&2
        exit 1
    fi
    dirty="$(git -C "$DG" status --porcelain --untracked-files=no 2>/dev/null)"
    if [ -n "$dirty" ]; then
        echo "STOP: the vendored doomgeneric tree has modified files -- the port" >&2
        echo "      compiles upstream UNMODIFIED; platform code belongs in" >&2
        echo "      sdk/apps/doom/, not in the vendored tree:" >&2
        echo "$dirty" | sed 's/^/        /' >&2
        exit 1
    fi
    echo "doomgeneric : $DG @ $head_rev (clean)"
else
    echo "doomgeneric : $DG (no .git -- pin $PIN NOT verified)"
fi

# --- libraries ----------------------------------------------------------------
. "$SDK/lib-stale.sh"
lib_ensure gemlib || exit 1
lib_ensure libm   || exit 1
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ]    || { echo "error: mintlib headers missing ($MINT)" >&2; exit 1; }
[ -d "$GEMTOP" ]  || { echo "error: gemlib headers missing ($GEMTOP)" >&2; exit 1; }
[ -d "$LIBMTOP" ] || { echo "error: OpenLibm headers missing ($LIBMTOP)" >&2; exit 1; }

# --- toolchain ----------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
NM="$BIN/llvm-nm"
SIZE="$BIN/llvm-size"
[ -x "$LLD" ] || { echo "error: missing LLVM tool: $LLD" >&2; exit 1; }

echo "clang : $($CLANG --version | head -1)"
echo "lld   : $LLD"
echo

# --- flags --------------------------------------------------------------------
# The NewTOS freestanding set shared with the other build-*.sh, -O2 as in
# build-qed.sh, -D__NEWTOS__ for the retargeted gemlib headers. doomgeneric's
# own Makefile adds only -DNORMALUNIX -DLINUX -DSNDSERV -D_DEFAULT_SOURCE for
# its X11 target; none of them selects code we want (sound and networking stay
# off through doomfeatures.h), so they are not carried over.
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fno-pic -fno-builtin -nostdinc -D__NEWTOS__
        -DCMAP256 -DDOOMGENERIC_RESX=320 -DDOOMGENERIC_RESY=200
        -Wno-macro-redefined)
DOOM_INCS=(-I"$DGSRC" -I"$MINT" -I"$LIBM/include" -I"$LIBMTOP/include" -isystem "$RES")
PLAT_INCS=(-I"$DGSRC" -I"$GEMTOP" -I"$MINT" -isystem "$RES")
PLAT_DEFS=(-DDOOM_GEM_ARGS="$DOOM_ARGS" -DDOOM_GEM_STATS="$DOOM_STATS")

# --- manifest: upstream SRC_DOOM, the X11 platform file replaced by ours ------
LIST="$BUILD/sources.txt"
sed -n 's/^SRC_DOOM *= *//p' "$DGSRC/Makefile" | tr ' ' '\n' | grep -E '\.o$' \
    | sed 's/\.o$/.c/' | grep -v '^doomgeneric_xlib\.c$' > "$LIST"
n=$(wc -l < "$LIST" | tr -d ' ')
[ "$n" -gt 0 ] || { echo "error: could not read SRC_DOOM from $DGSRC/Makefile" >&2; exit 1; }
echo "doomgeneric manifest: $n upstream files + doomgeneric_gem.c (replaces doomgeneric_xlib.c)"

rm -f "$OBJ"/*.o "$OBJ"/*.err
ok=0; fail=0; warned=0
: > "$BUILD/failed.txt"
while IFS= read -r c; do
    [ -n "$c" ] || continue
    o="$OBJ/${c%.c}.o"; e="$OBJ/${c%.c}.err"
    if "$CLANG" "${CFLAGS[@]}" "${DOOM_INCS[@]}" -c "$DGSRC/$c" -o "$o" 2>"$e"; then
        ok=$((ok+1)); [ -s "$e" ] && warned=$((warned+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
    fi
done < "$LIST"

e="$OBJ/doomgeneric_gem.err"
if "$CLANG" "${CFLAGS[@]}" -Wall -Wextra "${PLAT_DEFS[@]}" "${PLAT_INCS[@]}" \
        -c "$PORT/doomgeneric_gem.c" -o "$OBJ/doomgeneric_gem.o" 2>"$e"; then
    ok=$((ok+1))
    if [ -s "$e" ]; then echo "platform layer warnings:"; sed 's/^/    /' "$e"; fi
else
    echo "doomgeneric_gem.c" >> "$BUILD/failed.txt"; fail=$((fail+1))
fi

echo "compiled OK=$ok  FAILED=$fail  (upstream files with compiler warnings: $warned, see $OBJ/*.err)"
if [ "$fail" -ne 0 ]; then
    while IFS= read -r f; do
        echo "  === $f ==="
        sed 's/^/      /' "$OBJ/${f%.c}.err" 2>/dev/null | head -20
    done < "$BUILD/failed.txt"
    echo "STOP: an upstream file failing to compile is a TOOLCHAIN gap, never a doomgeneric edit."
    exit 1
fi

# --- CRT + link ---------------------------------------------------------------
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1
rm -f "$OUT"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why.txt" \
    "$BUILD/crt0.o" "$OBJ"/*.o \
    --start-group "$GEM/gemlib.a" "$LIBM/libm.a" "$LIBC/libc.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
if [ $? -ne 0 ]; then
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link.err" | awk '{print $3}' | sort -u | sed 's/^/    /'
    if grep -q "posix__system.o" "$BUILD/link.err"; then
        echo
        echo "KNOWN LIBC GAP: doomgeneric's i_system.c (ZenityErrorBox, reached from"
        echo "I_Error only when stdout is not a tty) calls system(). libc.a's system()"
        echo "links against spawnvp/vfork/wait4 pieces that do not build for LP64"
        echo "(sdk/libc/expected-failures.txt: spawnve.c/spawnvp.c; docs/sdk/"
        echo "07-mintlib-limits.md). The fix belongs in the libc, not in doomgeneric"
        echo "and not in this port -- see TASK/PORT06-STATUS-UNFINISHED.MD."
    fi
    exit 1
fi
u=$("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)
echo
echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes, undefined symbols: $u)"
[ -x "$SIZE" ] && "$SIZE" "$OUT"
# The TPA budget this binary leaves for malloc (doomgeneric_gem.c "MEMORY"):
# _end is the first free TPA byte; the TPA is 4 MiB with a 64 KiB stack reserve
# (sdk/libc/newtos-arch/heap_init.c).
end=$("$NM" "$OUT" | awk '$3 == "_end" {print $1}')
[ -n "$end" ] && echo "_end = 0x$end  -> malloc arena ~ $(( (0x400000 - 0x$end - 65536) / 1024 )) KiB"
echo "DOOM_ARGS  = $DOOM_ARGS"
echo "DOOM_STATS = $DOOM_STATS"
