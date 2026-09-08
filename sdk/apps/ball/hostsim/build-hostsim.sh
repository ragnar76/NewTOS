#!/usr/bin/env bash
#
# build-hostsim.sh -- build the bouncing-ball demo's host simulation.
#
# Two binaries, one per composition depth, because the depth is a compile-time
# property of the target (see ball.h) and both halves want exercising:
#
#   sim8    BALL_TRUECOLOR=0, the palette composition the Atari and the Pi use
#   sim32   BALL_TRUECOLOR=1, the RGB composition a truecolour card wants
#
# Both include ball.c and ball-tex.c verbatim -- the demo's own source, not a
# copy of it -- so what is measured here is what ships. See sim.c for what the
# virtual clock does and why.
#
# These two binaries carry the HOST's floating point, which is the Pi's and
# not the Atari's -- arm64 clang contracts a*b+c into an FMA, and the m68k
# compiler evaluates every float expression at 80 bits. sim.c's header has the
# one-line recipe for an exact m68k arithmetic model (clang -arch x86_64
# -ffp-eval-method=extended); use it before blaming an Atari-only difference
# on arithmetic.
#
#   output : sdk/apps/ball/hostsim/sim8, sim32
#
# Usage:  sdk/apps/ball/hostsim/build-hostsim.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BALL="$(cd "$HERE/.." && pwd)"
CC="${CC:-clang}"

command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }

CFLAGS=(-O2 -std=c99 -Wall -Wextra)
INCS=(-I"$HERE" -I"$BALL")

rc=0
for d in 0 1; do
    out="$HERE/sim$([ "$d" = 0 ] && echo 8 || echo 32)"
    if "$CC" "${CFLAGS[@]}" "${INCS[@]}" -DBALL_TRUECOLOR=$d \
             -o "$out" "$HERE/sim.c" "$BALL/ball-tex.c" 2>"$HERE/sim$d.err"; then
        [ -s "$HERE/sim$d.err" ] && { echo "warnings (BALL_TRUECOLOR=$d):";
                                      sed 's/^/    /' "$HERE/sim$d.err"; }
        echo "  CC  BALL_TRUECOLOR=$d -> $out"
    else
        echo "BUILD FAILED (BALL_TRUECOLOR=$d):"
        sed 's/^/    /' "$HERE/sim$d.err"
        rc=1
    fi
done
exit $rc
