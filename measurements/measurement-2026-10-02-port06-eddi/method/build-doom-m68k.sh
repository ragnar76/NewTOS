#!/usr/bin/env bash
#
# build-doom-m68k.sh <outdir>
#
# MEASUREMENT HELPER for the PORT06 EdDI step, NOT the delivered m68k build
# (that is the next PORT06 step). Compiles the same manifest as
# sdk/apps/build-doom.sh -- upstream SRC_DOOM with doomgeneric_xlib.c replaced
# by sdk/apps/doom/doomgeneric_gem.c -- with the classic cross-mint GCC 7.5.0
# for a 68030 with FPU (-m68020-60, the multilib the sys-root ships; TT030 /
# Falcon030), against the cross-mint mintlib and sdk/gemlib/gemlib-m68k.a.
# The vendored doomgeneric tree is read only. Byte order comes from
# doomgeneric's own i_swap.h (__BYTE_ORDER__), nothing set here.
# <math.h>: the cross-mint sys-root has none (mintlib ships no math.h); the
# upstream files get the SDK's header (sdk/libm/include over OpenLibm's), as
# in build-doom.sh. DOOM calls no libm function (the link needs no libm).
set -uo pipefail
OUT="${1:?usage: build-doom-m68k.sh <outdir>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
DGSRC="$WS/Applications/doomgeneric/doomgeneric"
XCC=/opt/cross-mint/bin/m68k-atari-mint-gcc
GEMTOP="$WS/sdk/gemlib/gemlib-0.44.0-m68k"
GEMLIB="$WS/sdk/gemlib/gemlib-m68k.a"
DOOM_ARGS="${DOOM_ARGS:-\"-mb\",\"4\"}"
DOOM_STATS="${DOOM_STATS:-5}"
mkdir -p "$OUT/obj"
echo "gcc : $($XCC --version | head -1)"
CF=(-m68020-60 -O2 -fomit-frame-pointer -DCMAP256 -DDOOMGENERIC_RESX=320 -DDOOMGENERIC_RESY=200)
sed -n 's/^SRC_DOOM *= *//p' "$DGSRC/Makefile" | tr ' ' '\n' | grep -E '\.o$' \
    | sed 's/\.o$/.c/' | grep -v '^doomgeneric_xlib\.c$' > "$OUT/sources.txt"
ok=0; fail=0; warned=0
while IFS= read -r c; do
    [ -n "$c" ] || continue
    if "$XCC" "${CF[@]}" -I"$DGSRC" -I"$WS/sdk/libm/include" -I"$WS/sdk/libm/openlibm-0.8.8/include" -c "$DGSRC/$c" -o "$OUT/obj/${c%.c}.o" 2>"$OUT/obj/${c%.c}.err"; then
        ok=$((ok+1)); [ -s "$OUT/obj/${c%.c}.err" ] && warned=$((warned+1))
    else
        fail=$((fail+1)); echo "FAILED: $c"; head -5 "$OUT/obj/${c%.c}.err"
    fi
done < "$OUT/sources.txt"
if "$XCC" "${CF[@]}" -Wall -Wextra -DDOOM_GEM_ARGS="$DOOM_ARGS" -DDOOM_GEM_STATS="$DOOM_STATS" \
        -I"$DGSRC" -I"$GEMTOP" -c "$WS/sdk/apps/doom/doomgeneric_gem.c" \
        -o "$OUT/obj/doomgeneric_gem.o" 2>"$OUT/obj/doomgeneric_gem.err"; then
    ok=$((ok+1))
    if [ -s "$OUT/obj/doomgeneric_gem.err" ]; then echo "platform layer warnings:"; cat "$OUT/obj/doomgeneric_gem.err"; else echo "platform layer: -Wall -Wextra clean"; fi
else
    fail=$((fail+1)); echo "FAILED: doomgeneric_gem.c"; cat "$OUT/obj/doomgeneric_gem.err"
fi
echo "compiled OK=$ok FAILED=$fail (upstream files with warnings: $warned)"
[ "$fail" -eq 0 ] || exit 1
"$XCC" -m68020-60 -o "$OUT/DOOM.PRG" "$OUT"/obj/*.o "$GEMLIB" 2>"$OUT/link.err"
rc=$?
if [ $rc -ne 0 ]; then echo "LINK FAILED:"; head -30 "$OUT/link.err"; exit 1; fi
echo "LINK OK -> $OUT/DOOM.PRG ($(wc -c < "$OUT/DOOM.PRG" | tr -d ' ') bytes)"
echo "DOOM_ARGS  = $DOOM_ARGS"
echo "DOOM_STATS = $DOOM_STATS"
