#!/usr/bin/env bash
#
# build-ball-freeze.sh -- the three diagnostic builds F1, F2, F3, for all
# three machines.
#
# There is an artefact on the ball that is visible ONLY IN MOTION: it is in no
# single frame, so a photograph does not show it and the host simulator's
# per-frame verifier calls every presented frame exact while it is happening.
# Two things move -- the ball across the screen and the texture around the
# ball -- and every attempt so far has correlated them. These builds separate
# them: each holds one of the two still. See the BALL_FREEZE commentary at the
# top of ball.c.
#
#   F1   ball STILL, texture STILL     the picture does not change at all
#   F2   ball STILL, texture TURNING   only the surface moves
#   F3   ball FLYING, texture STILL    only the position moves
#
# Measured on a Pi 4: F1 no, F2 no (but it exposed a page-flip seam that is
# NewTOS's, not the demo's -- see plat-newtos.c), F3 YES. So it is the travel.
# Two more ask what about the travel, both about the ball's ordered dither:
#
#   F4   no dither on the ball at all  it will band; that is expected
#   F5   the same dither, indexed in SCREEN space, not in the sphere map's
#
# F5 is the more valuable: F4 alone cannot tell "dithering is the problem"
# from "dithering that travels with the object is the problem". It costs the
# ball's inner loop 10 -> 18 m68k instructions a pixel and D*D*2 bytes.
#
# Run them back to back on one machine and say for each: is the artefact
# there, yes or no. One should be yes and two should be no; that says which
# half of the motion it lives in. There is nothing to read off, no number to
# note, and nothing is tuned.
#
# Each build prints, as the FIRST line of its log and of its console output,
# which of the three it is -- three runs on one machine are otherwise
# impossible to tell apart afterwards.
#
#   output : BALLF1.PRG  BALLF2.PRG  BALLF3.PRG    (Atari TT, 256 colours)
#            BALLF1TC.PRG BALLF2TC.PRG BALLF3TC.PRG (ST + PiStorm, truecolour)
#            BALLF1.NAP  BALLF2.NAP  BALLF3.NAP    (NewTOS on the Pi 4)
#
# The demo itself is NOT rebuilt by this script and is not affected: with
# BALL_FREEZE unset the compiler emits the same instructions it emitted before
# the switch existed, checked by comparing generated assembly on both
# architectures at both composition depths.
#
# Usage:  sdk/apps/ball/build-ball-freeze.sh [1..5 ...]       (default: all)
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LEVELS=("$@")
[ ${#LEVELS[@]} -eq 0 ] && LEVELS=(1 2 3 4 5)

rc=0
for f in "${LEVELS[@]}"; do
    case "$f" in
      1|2|3|4|5) ;;
      *) echo "error: freeze level must be 1..5 (got '$f')" >&2; exit 1 ;;
    esac

    echo "==================== F$f ===================="
    export EXTRA_CFLAGS="-DBALL_FREEZE=$f"

    if [ -x "$HERE/build-ball-tt.sh" ]; then
        "$HERE/build-ball-tt.sh" "$HERE/BALLF$f.PRG" >/dev/null 2>&1 \
            && echo "  TT       -> BALLF$f.PRG" \
            || { echo "  TT       FAILED"; rc=1; }
    fi
    if [ -x "$HERE/build-ball-pistorm.sh" ]; then
        "$HERE/build-ball-pistorm.sh" "$HERE/BALLF${f}TC.PRG" >/dev/null 2>&1 \
            && echo "  PiStorm  -> BALLF${f}TC.PRG" \
            || { echo "  PiStorm  FAILED"; rc=1; }
    fi
    if [ -x "$HERE/build-ball.sh" ]; then
        "$HERE/build-ball.sh" "$HERE/BALLF$f.NAP" >/dev/null 2>&1 \
            && echo "  Pi 4     -> BALLF$f.NAP" \
            || { echo "  Pi 4     FAILED"; rc=1; }
    fi
    unset EXTRA_CFLAGS
done

echo
if [ $rc -eq 0 ]; then
    echo "All diagnostic builds made. The demo's own binaries are untouched;"
    echo "rebuild them with build-ball.sh / -tt.sh / -pistorm.sh as usual."
else
    echo "At least one build failed -- rerun the individual script to see why."
fi
exit $rc
