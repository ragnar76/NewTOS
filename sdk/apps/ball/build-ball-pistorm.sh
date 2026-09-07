#!/usr/bin/env bash
#
# build-ball-pistorm.sh -- build BALLTC.PRG, the bouncing-ball demo, for an
# Atari with a PiStorm and an ET4000 in a TRUECOLOUR (32 bits per pixel) mode.
#
# The truecolour sibling of build-ball-tt.sh. It differs in exactly two
# places, and both of them are the point:
#
#                    build-ball-tt.sh              build-ball-pistorm.sh
#   platform half    plat-tt.c    (palette)        plat-pistorm.c  (RGB)
#   composition      -                             -DBALL_TRUECOLOR=1
#   product          BALL.PRG                      BALLTC.PRG
#
# That -D is the whole mechanism: ball.h turns it into the width of a
# composition pixel and the core is TOLD, at compile time, what the target
# is. Nothing in ball.c or ball-tex.c differs between the two builds -- they
# are byte-identical inputs to both, as they are to the Pi's build.
#
# ---------------------------------------------------------------------------
# NEITHER THIS SCRIPT NOR plat-pistorm.c HAS EVER BEEN RUN
# ---------------------------------------------------------------------------
# They were written while the m68k cross toolchain was quarantined and could
# not be invoked at all. plat-pistorm.c has been type-checked against the
# real gemlib and cross-tree headers with a different compiler, and ball.c
# has been built at both depths for AArch64 and rendered at both depths in a
# host simulation -- but the m68k product does not exist yet and the first
# real run is still ahead. See the header of plat-pistorm.c for the four
# things about the card that only that run can settle.
#
#   objs   : sdk/apps/ball/build-pistorm/
#   output : sdk/apps/ball/BALLTC.PRG  (GEMDOS executable, m68k)
#
# Usage:  sdk/apps/ball/build-ball-pistorm.sh [output.PRG]
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"      # sdk/apps/ball
SDK="$(cd "$HERE/../.." && pwd)"                          # sdk
OUT="${1:-$HERE/BALLTC.PRG}"
GEM="$SDK/gemlib"
GEMTOP="$GEM/gemlib-0.44.0-m68k"                          # pristine m68k tree
GEMLIB="$GEM/gemlib-m68k.a"
BUILD="$HERE/build-pistorm"
mkdir -p "$BUILD"

for s in ball.c ball-tex.c plat-pistorm.c ball.h ball-tex.h; do
    [ -f "$HERE/$s" ] || { echo "error: $HERE/$s missing" >&2; exit 1; }
done

# --- toolchain ---------------------------------------------------------------
# The CLASSIC m68k-atari-mint GCC (GEMDOS executables), NOT mintelf.
XPREFIX="${XPREFIX:-/opt/cross-mint/bin/m68k-atari-mint-}"
XCC="${XCC:-${XPREFIX}gcc}"
XSIZE="${XSIZE:-${XPREFIX}size}"
XNM="${XNM:-${XPREFIX}nm}"
if [ ! -x "$XCC" ]; then
    echo "error: m68k cross compiler not found: $XCC" >&2
    echo "       set XPREFIX=/path/to/m68k-atari-mint- and try again." >&2
    exit 1
fi

# --- is a cross tool actually usable? ----------------------------------------
# On macOS an unsigned cross tool that still carries com.apple.quarantine does
# not fail when it is run non-interactively -- it STALLS, waiting for a
# Gatekeeper prompt that no one will ever see. That looks exactly like a hung
# build, so ask each tool a trivial question under a watchdog first and say
# plainly what is wrong instead of hanging. (Approval is per binary, so some
# tools of the same install can work while others stall.)
tool_responds() {
    local p w rc
    "$1" --version >/dev/null 2>&1 &
    p=$!
    ( sleep 5; kill -9 $p 2>/dev/null ) &
    w=$!
    { wait $p; rc=$?; } 2>/dev/null
    kill $w 2>/dev/null; wait $w 2>/dev/null
    return $rc
}

tool_or_die() {
    tool_responds "$1" && return 0
    echo "error: $1 does not answer --version." >&2
    echo "       If this is macOS, the tool is most likely quarantined and is" >&2
    echo "       stalling on a Gatekeeper prompt that a non-interactive shell" >&2
    echo "       never gets. Clear it once, for the whole toolchain:" >&2
    echo "           sudo xattr -dr com.apple.quarantine $(dirname "$(dirname "$1")")" >&2
    echo "       (or approve it in System Settings -> Privacy & Security)." >&2
    exit 1
}

tool_or_die "$XCC"

case "$("$XCC" -dumpmachine)" in
  m68k-atari-mint) ;;
  *) echo "error: $XCC is $("$XCC" -dumpmachine), not the classic m68k-atari-mint" >&2
     echo "       a 'mintelf' toolchain will not link against this tree's libraries." >&2
     exit 1 ;;
esac

# --- gemlib for m68k ----------------------------------------------------------
if [ ! -f "$GEMLIB" ]; then
  echo "gemlib-m68k.a absent -- running build-gemlib-m68k.sh first…"
  "$GEM/build-gemlib-m68k.sh" >"$BUILD/gemlib.log" 2>&1 \
      || { echo "error: build-gemlib-m68k.sh failed, see $BUILD/gemlib.log" >&2; exit 1; }
fi
[ -d "$GEMTOP" ] || { echo "error: gemlib headers missing ($GEMTOP)" >&2; exit 1; }

# --- flags --------------------------------------------------------------------
# -funsigned-char matches the AArch64 build, so that ball.c's arithmetic on
# palette indices and texels means the same thing on both machines.
# -DBALL_TRUECOLOR=1 is the one flag that separates this build from the TT's.
# -funsigned-char matches the AArch64 build, so that ball.c's arithmetic on
# texels means the same thing on both machines.
CFLAGS=(-m68020-60 -O2 -fomit-frame-pointer -funsigned-char
        -std=c99 -Wall -Wextra -DBALL_TRUECOLOR=1)
INCS=(-I"$HERE" -I"$GEMTOP")

echo "gcc  : $("$XCC" --version | head -1)"
echo "cpu  : -m68020-60  (multilib $("$XCC" -m68020-60 -print-multi-directory), hardware FPU)"
echo "src  : ball.c + ball-tex.c + plat-pistorm.c  (BALL_TRUECOLOR=1)"
echo "gem  : $GEMLIB"
echo "out  : $OUT"
echo

rm -f "$BUILD"/*.o
for s in ball.c ball-tex.c plat-pistorm.c; do
    o="$BUILD/${s%.c}.o"
    if ! "$XCC" "${CFLAGS[@]}" "${INCS[@]}" -c "$HERE/$s" -o "$o" 2>"$BUILD/${s%.c}.err"; then
        echo "COMPILE FAILED ($s):"
        sed 's/^/    /' "$BUILD/${s%.c}.err"
        exit 1
    fi
    if [ -s "$BUILD/${s%.c}.err" ]; then
        echo "  CC  $s   (warnings:)"
        sed 's/^/      /' "$BUILD/${s%.c}.err"
    else
        echo "  CC  $s"
    fi
done

rm -f "$OUT"
"$XCC" -m68020-60 -o "$OUT" \
    "$BUILD/ball.o" "$BUILD/ball-tex.o" "$BUILD/plat-pistorm.o" \
    "$GEMLIB" 2>"$BUILD/link.err"
lr=$?
if [ $lr -ne 0 ]; then
    echo
    echo "LINK FAILED:"
    sed 's/^/    /' "$BUILD/link.err"
    exit 1
fi
[ -s "$BUILD/link.err" ] && { echo "link warnings:"; sed 's/^/    /' "$BUILD/link.err"; }

echo
echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
"$XSIZE" "$OUT"
if tool_responds "$XNM"; then
    u=$("$XNM" -u "$OUT" 2>/dev/null | grep -c . || true)
    echo "undefined symbols in the final image (goal 0): $u"
    if [ "$u" -ne 0 ]; then
        echo "  residual undefined:"; "$XNM" -u "$OUT" | sed 's/^/    /'
        exit 1
    fi
else
    echo "undefined symbols: NOT CHECKED ($XNM does not answer; see above)"
fi
echo
echo "Copy BALLTC.PRG into the root of C: and double-click it from the desktop."
echo "It needs a 32-bit-per-pixel screen and refuses to run on anything else;"
echo "for a 256-colour mode use BALL.PRG. It writes C:\\BALL.LOG, and the last"
echo "line of that log is what the format probes decided -- read it first."
