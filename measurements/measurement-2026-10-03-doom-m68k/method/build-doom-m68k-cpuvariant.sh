#!/usr/bin/env bash
# MEASUREMENT COPY of sdk/apps/build-doom-m68k.sh (2026-10-03), the only change:
# CPU=(${CPU_OVERRIDE}) so the -m68020-60 build can be compared with the shipped
# -m68030 -m68881 one (both with DOOM_STATS=5). Not a delivered script.
#
# build-doom-m68k.sh -- compile doomgeneric (id Software's DOOM, GPLv2, via
# https://github.com/ozkl/doomgeneric) into an m68k GEM program, DOOM.PRG,
# for an Atari TT030 (68030 + 68882), with or without a graphics card (NOVA).
# TASK/PORT06: ONE source for NewTOS and for the TT -- this is the TT half of
# sdk/apps/build-doom.sh, same tree, same platform file, same checks.
#
# DOOM.PRG IS FOR THE ATARI, NOT FOR NEWTOS
#   .PRG is the Atari convention for a GEM program, so the TT gets a .PRG.
#   On NewTOS, .PRG is RESERVED for the future m68k userland emulator (memory
#   note "own-extensions-separate-aarch64-from-m68k"; native programs are
#   .NAP/.NCP). Do not put this file on a NewTOS card -- the NewTOS build is
#   DOOM.NAP from sdk/apps/build-doom.sh.
#
# SOURCES (identical to build-doom.sh)
#   Applications/doomgeneric/        vendored doomgeneric, pinned at commit
#                                    dcb7a8dbc7a16ce3dda29382ac9aae9d77d21284,
#                                    untracked and NEVER EDITED; the script
#                                    refuses another commit or a modified file.
#   sdk/apps/doom/doomgeneric_gem.c  our platform layer (AES/VDI/BIOS/XBIOS/
#                                    GEMDOS only), in place of
#                                    doomgeneric_xlib.c in upstream's SRC_DOOM.
#
# TOOLCHAIN AND LIBRARIES
#   /opt/cross-mint m68k-atari-mint-gcc 7.5.0 (the classic `mint` a.out-PRG
#   flavour), its sys-root mintlib (libc.a) and libgcc, and
#   sdk/gemlib/gemlib-m68k.a (pristine gemlib-0.44.0, trap #2; built by
#   sdk/gemlib/build-gemlib-m68k.sh, run here if the archive is missing or
#   older than that script).
#   <math.h>: the cross-mint sys-root has none (mintlib ships no math.h), so
#   the upstream files get the SDK's adapter header (sdk/libm/include over
#   OpenLibm's headers), as build-doom.sh does. Only the header -- DOOM calls
#   no libm function; the link takes no libm and must end with 0 undefined.
#
# CPU / FPU: -m68030 -m68881
#   * -m68030 selects the m68020-60 multilib (gcc -print-multi-directory), the
#     only 020+ libc.a/libgcc.a the sys-root ships; both are hard-float
#     (__HAVE_68881__), so the objects and the libraries agree on the float
#     ABI (float/double results in fp0). -m68881 is the default for -m68030;
#     it is written out because the libraries require it -- -msoft-float would
#     STILL pick the hard-float m68020-60 libc (checked), i.e. a silent ABI
#     mismatch. There is no -m68882 in gcc: the 68882 is a 68881 superset.
#   * -m68030 rather than -m68020-60: -m68020-60 must avoid what the 68060
#     lacks in hardware, among it the 32x32->64 MULS.L. DOOM's hottest
#     arithmetic is FixedMul (m_fixed.c, an int64_t product >> 16): with
#     -m68020-60 it compiles to a call of libgcc's __muldi3, with -m68030 to
#     one `muls.l <ea>,dh:dl` (both verified on the gcc -S output). The target
#     is a TT (68030) first and a Falcon030 second -- the same CPU -- so the
#     030 code is the right one. A 68040 runs it unchanged; a 68060 traps the
#     64-bit MULS.L to its 060SP software package (correct, slower), so a
#     CT60 is served but not tuned for.
#   * An FPU is REQUIRED (every TT has its 68882; a Falcon needs the 68882
#     socket populated): the hard-float libc's float code (printf/scanf/
#     strtod) uses FPU instructions, and so does the one float spot in DOOM
#     (V_DrawMouseSpeedBox); the game itself computes in fixed point.
#
# PROGRAM HEADER FLAGS: --mfastload --mfastram --mfastalloc (= 0x7)
#   mintlib's malloc() takes its memory from GEMDOS Malloc() (unix/sbrk.c
#   __sbrk, _split_mem set by mintlib/crtinit.c for the default _stksize), and
#   on TOS 3/4 Malloc() serves TT-RAM only if the program header carries
#   PF_TTRAMMEM (4), ST-RAM otherwise (EmuTOS bdos/umem.c xmalloc:
#   MX_PREFTTRAM vs MX_STRAM). PF_TTRAMLOAD (2) puts the program itself into
#   TT-RAM (EmuTOS bdos/proc.c). Nothing of DOOM's needs ST-RAM (no DMA, no
#   sound; the picture reaches the screen through vro_cpyfm, PORT06 "ST-RAM
#   ist auch auf dem TT kein Thema"), and TT-RAM is the TT's fast memory
#   without video contention. Without TT-RAM both fall back to ST-RAM.
#   PF_FASTLOAD (1) only skips clearing the free TPA beyond the BSS, which
#   neither mintlib's malloc nor DOOM's zone relies on. 0x7 is also what the
#   classic cross-mint ld writes when given nothing (checked); the flags are
#   spelled out so the header does not depend on a toolchain default.
#
# ZONE SIZE: DOOM_ARGS default '"-mb","6"'
#   NewTOS' "-mb 2" exists only because NewTOS' TPA is 4 MiB
#   (doomgeneric_gem.c "MEMORY"); the TT has no such cap. 6 MiB is DOOM's own
#   value, default AND minimum (doomgeneric i_system.c DEFAULT_RAM/MIN_RAM,
#   the "mb_used = 6" of id's linuxdoom). It is passed explicitly rather than
#   left out because the platform appends DOOM_GEM_ARGS to every start and an
#   empty list would be an empty array initializer; the effect is the same
#   as no -mb. With the header flags above the zone is malloc'ed from TT-RAM
#   when there is TT-RAM. A start with "-mb N" wins (DOOM takes the first
#   occurrence), e.g. "-mb 3" on a TT with 4 MiB ST-RAM and no TT-RAM.
#
# Environment knobs:
#   DOOM_ARGS='"-mb","6"'   extra arguments appended to every start (above)
#   DOOM_STATS=<seconds>    cost probe + fps line, as in build-doom.sh;
#                           default 0 (the shipped build). Measurement runs
#                           pass e.g. DOOM_STATS=5.
#   OUT=<path>              output file (default sdk/apps/doom/build/DOOM.PRG)
#   BUILD=<dir>             object directory (default sdk/apps/doom/build/m68k;
#                           separate from build-doom.sh's build/obj, which
#                           that script empties on every run)
#
# Usage:  sdk/apps/build-doom-m68k.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../sdk" && pwd)"   # measurement copy: method/ -> sdk
WS="$(cd "$SDK/.." && pwd)"
PORT="$SDK/apps/doom"                                         # our platform layer
DG="${DOOMGENERIC:-$WS/Applications/doomgeneric}"             # vendored tree
DGSRC="$DG/doomgeneric"
PIN="dcb7a8dbc7a16ce3dda29382ac9aae9d77d21284"
GEMDIR="$SDK/gemlib"
GEMTOP="$GEMDIR/gemlib-0.44.0-m68k"
GEMLIB="$GEMDIR/gemlib-m68k.a"
LIBM="$SDK/libm"
LIBMTOP="$LIBM/openlibm-0.8.8"
BUILD="${BUILD:-$PORT/build/m68k}"
OBJ="$BUILD/obj"
OUT="${OUT:-$PORT/build/DOOM.PRG}"
DOOM_ARGS="${DOOM_ARGS:-\"-mb\",\"6\"}"
DOOM_STATS="${DOOM_STATS:-0}"
mkdir -p "$OBJ" "$(dirname "$OUT")"

[ -d "$DGSRC" ] || { echo "error: doomgeneric source missing ($DGSRC)" >&2; exit 1; }
[ -f "$PORT/doomgeneric_gem.c" ] || { echo "error: platform layer missing ($PORT/doomgeneric_gem.c)" >&2; exit 1; }

# --- the vendored tree must be the pinned, unmodified upstream ---------------
# (the same read-only check as build-doom.sh)
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

# --- toolchain ----------------------------------------------------------------
XPREFIX="${XPREFIX:-/opt/cross-mint/bin/m68k-atari-mint-}"
XCC="${XPREFIX}gcc"
XNM="${XPREFIX}nm"
for t in "$XCC" "$XNM"; do
    [ -x "$t" ] || { echo "error: cross tool missing: $t (classic m68k-atari-mint GCC)" >&2; exit 1; }
done
CPU=(${CPU_OVERRIDE:--m68030 -m68881})
mdir="$("$XCC" "${CPU[@]}" -print-multi-directory)"
if [ "$mdir" != "m68020-60" ]; then
    echo "STOP: ${CPU[*]} picks multilib '$mdir', expected the hard-float m68020-60." >&2
    exit 1
fi
"$XCC" "${CPU[@]}" -dM -E -x c /dev/null | grep -q '__HAVE_68881__' \
    || { echo "STOP: ${CPU[*]} does not predefine __HAVE_68881__ (soft float?)" >&2; exit 1; }
echo "gcc : $("$XCC" --version | head -1)"
echo "cpu : ${CPU[*]} -> multilib $mdir ($("$XCC" "${CPU[@]}" -print-libgcc-file-name)), hard float"

# --- libraries ----------------------------------------------------------------
if [ ! -f "$GEMLIB" ] || [ "$GEMDIR/build-gemlib-m68k.sh" -nt "$GEMLIB" ]; then
    echo "gemlib-m68k.a missing or older than its build script -- building it"
    "$GEMDIR/build-gemlib-m68k.sh" > "$BUILD/build-gemlib-m68k.log" 2>&1 \
        || { echo "error: build-gemlib-m68k.sh failed, see $BUILD/build-gemlib-m68k.log" >&2; exit 1; }
fi
[ -d "$GEMTOP" ]  || { echo "error: m68k gemlib headers missing ($GEMTOP)" >&2; exit 1; }
[ -d "$LIBMTOP" ] || { echo "error: OpenLibm headers missing ($LIBMTOP)" >&2; exit 1; }
echo "gemlib : $GEMLIB"
echo

# --- flags --------------------------------------------------------------------
# -O2 -fomit-frame-pointer as gemlib-m68k.a; the doomgeneric configuration is
# build-doom.sh's (-DCMAP256, 320x200 output buffer; byte order from
# doomgeneric's own i_swap.h via __BYTE_ORDER__ -> big endian here).
CFLAGS=("${CPU[@]}" -O2 -fomit-frame-pointer
        -DCMAP256 -DDOOMGENERIC_RESX=320 -DDOOMGENERIC_RESY=200)
DOOM_INCS=(-I"$DGSRC" -I"$LIBM/include" -I"$LIBMTOP/include")
PLAT_INCS=(-I"$DGSRC" -I"$GEMTOP")
PLAT_DEFS=(-DDOOM_GEM_ARGS="$DOOM_ARGS" -DDOOM_GEM_STATS="$DOOM_STATS")
LDFLAGS=("${CPU[@]}" -Wl,--mfastload -Wl,--mfastram -Wl,--mfastalloc)

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
    if "$XCC" "${CFLAGS[@]}" "${DOOM_INCS[@]}" -c "$DGSRC/$c" -o "$o" 2>"$e"; then
        ok=$((ok+1)); [ -s "$e" ] && warned=$((warned+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
    fi
done < "$LIST"

e="$OBJ/doomgeneric_gem.err"
if "$XCC" "${CFLAGS[@]}" -Wall -Wextra "${PLAT_DEFS[@]}" "${PLAT_INCS[@]}" \
        -c "$PORT/doomgeneric_gem.c" -o "$OBJ/doomgeneric_gem.o" 2>"$e"; then
    ok=$((ok+1))
    if [ -s "$e" ]; then echo "platform layer warnings:"; sed 's/^/    /' "$e"; else echo "platform layer: -Wall -Wextra clean"; fi
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

# --- link (gcc driver: crt0.o, mintlib libc.a and libgcc.a of the multilib) ---
rm -f "$OUT"
"$XCC" "${LDFLAGS[@]}" -o "$OUT" "$OBJ"/*.o "$GEMLIB" 2>"$BUILD/link.err"
if [ $? -ne 0 ]; then
    echo "LINK FAILED:"
    grep -E "undefined reference" "$BUILD/link.err" | sed -E "s/.*undefined reference to .(.*)'.*/\1/" \
        | sort -u | sed 's/^/    undefined: /'
    head -20 "$BUILD/link.err"
    exit 1
fi
[ -s "$BUILD/link.err" ] && { echo "linker messages:"; sed 's/^/    /' "$BUILD/link.err"; }

# --- report -------------------------------------------------------------------
u=$("$XNM" -u "$OUT" 2>/dev/null | grep -c . || true)
echo
echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes, undefined symbols: $u)"
# GEMDOS program header: 0x601a, text/data/bss/sym lengths, reserved, flags.
hdr=$(od -An -tx1 -N28 "$OUT" | tr -d ' \n')
echo "PRG header: magic 0x${hdr:0:4} text 0x${hdr:4:8} data 0x${hdr:12:8} bss 0x${hdr:20:8} flags 0x${hdr:44:8}"
echo "            (flags: 1 fastload, 2 load into TT-RAM, 4 Malloc from TT-RAM)"
echo "DOOM_ARGS  = $DOOM_ARGS"
echo "DOOM_STATS = $DOOM_STATS"
