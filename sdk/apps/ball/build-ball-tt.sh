#!/usr/bin/env bash
#
# build-ball-tt.sh -- build BALL.PRG, the bouncing-ball demo, for a real Atari
# TT030 with a NOVA (ATI mach64) graphics card.
#
# The m68k sibling of build-ball.sh. Same three questions, different answers:
#
#                    build-ball.sh (Pi 4)          build-ball-tt.sh (TT)
#   compiler         clang -target aarch64...      m68k-atari-mint-gcc
#   CPU              AArch64, LP64                 -m68020-60, hardware FPU
#   libc             sdk/libc/libc.a (ported)      the cross tree's mintlib
#   gemlib           sdk/gemlib/gemlib.a (svc #2)  sdk/gemlib/gemlib-m68k.a
#                                                  (pristine, trap #2)
#   platform half    plat-newtos.c                 plat-tt.c
#   product          BALL.NAP (ELF64)              BALL.PRG (GEMDOS)
#
# ball.c and ball-tex.c are byte-identical inputs to both. That is the point:
# everything that differs between the two machines is behind ball.h.
#
# -m68020-60 is deliberate and is the ONLY CPU setting here:
#   * it is the multilib the cross tree actually ships a libc.a for;
#   * it predefines __M68881__ / __HAVE_68881__, i.e. HARDWARE floating point,
#     which every TT has and which this demo leans on heavily;
#   * it is 68020..68060 code, so it runs on the TT030 target and on a Falcon
#     or an accelerated machine without a second build.
# It does NOT emit the 68881-only transcendental instructions -- which costs
# nothing, because ball.c brings its own sqrt/asin/sin and asks libm for
# nothing.
#
# -O2, not -Os: this one is a performance demo.
#
#   objs   : sdk/apps/ball/build-tt/
#   output : sdk/apps/ball/BALL.PRG  (GEMDOS executable, m68k)
#
# Usage:  sdk/apps/ball/build-ball-tt.sh [output.PRG]
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"      # sdk/apps/ball
SDK="$(cd "$HERE/../.." && pwd)"                          # sdk
OUT="${1:-$HERE/BALL.PRG}"
GEM="$SDK/gemlib"
GEMTOP="$GEM/gemlib-0.44.0-m68k"                          # pristine m68k tree
GEMLIB="$GEM/gemlib-m68k.a"
BUILD="$HERE/build-tt"
mkdir -p "$BUILD"

for s in ball.c ball-tex.c plat-tt.c ball.h ball-tex.h; do
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
# EXTRA_CFLAGS lets a caller add a compile-time switch without editing this
# script -- build-ball-freeze.sh uses it for -DBALL_FREEZE. Empty by default,
# and an empty expansion adds no argument at all.
CFLAGS=(-m68020-60 -O2 -fomit-frame-pointer -funsigned-char
        -std=c99 -Wall -Wextra ${EXTRA_CFLAGS:-})
INCS=(-I"$HERE" -I"$GEMTOP")

echo "gcc  : $("$XCC" --version | head -1)"
echo "cpu  : -m68020-60  (multilib $("$XCC" -m68020-60 -print-multi-directory), hardware FPU)"
echo "src  : ball.c + ball-tex.c + plat-tt.c"
echo "gem  : $GEMLIB"
echo "out  : $OUT"
echo

rm -f "$BUILD"/*.o
for s in ball.c ball-tex.c plat-tt.c; do
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
    "$BUILD/ball.o" "$BUILD/ball-tex.o" "$BUILD/plat-tt.o" \
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
echo "Copy BALL.PRG into the root of C: and double-click it from the desktop."
echo "It needs a 640x480 256-colour screen; it writes C:\\BALL.LOG."
