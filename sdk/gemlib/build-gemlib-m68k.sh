#!/usr/bin/env bash
#
# build-gemlib-m68k.sh -- build gemlib-0.44.0 into gemlib-m68k.a for a real
# Atari (m68k, TOS/MiNT), using the cross-mint toolchain.
#
# This is the m68k SIBLING of build-gemlib.sh, and the difference between them
# is the whole point of the exercise:
#
#   build-gemlib.sh       AArch64 / NewTOS. Applies sdk/gemlib/overlay/, which
#                         RETARGETS gemlib's one OS boundary -- the `trap #2`
#                         primitive -- onto NewTOS's `svc #2` (ADR-019).
#   build-gemlib-m68k.sh  m68k / TOS. Applies NOTHING. On the machine gemlib
#                         was written for, the pristine `trap #2` inline asm in
#                         gem_vdiP.h / gem_aesP.h IS the correct primitive, and
#                         our overlay must be kept away from it.
#
# So this script extracts the vendored pristine tarball into a SEPARATE tree
# (gemlib-0.44.0-m68k/, gitignored) and never touches the overlaid one. Both
# libraries are built from the same vendored source, which is exactly what
# makes an m68k build and a NewTOS build of the same program comparable. The
# consumer that motivated this library -- the bouncing-ball demo -- left this
# tree on 2026-09-09 for /Volumes/Daten/Development/ball-demo/; the recipe
# stays here because the SDK is where a cross library belongs, and because
# the m68k userland emulator will want it.
#
# No prebuilt m68k gemlib package is used and nothing is installed into the
# shared toolchain at /opt/cross-mint -- everything this script writes stays
# inside the repository.
#
# Toolchain: the CLASSIC m68k-atari-mint GCC (a.out-ish `mint` flavour), NOT
# `mintelf`. -m68020-60 is the multilib whose libc.a the sys-root ships and it
# predefines __M68881__ / __HAVE_68881__, i.e. hardware floating point -- which
# is what a TT030 has, and what any m68k GEM program of that era expects.
#
#   output : sdk/gemlib/gemlib-m68k.a
#
# Usage:  sdk/gemlib/build-gemlib-m68k.sh
set -uo pipefail

GEM="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # sdk/gemlib
TOP="$GEM/gemlib-0.44.0-m68k"                                # pristine, m68k
BUILD="$GEM/build-m68k"
OBJ="$BUILD/obj"
OUT="$GEM/gemlib-m68k.a"
mkdir -p "$OBJ"

# --- toolchain ---------------------------------------------------------------
XPREFIX="${XPREFIX:-/opt/cross-mint/bin/m68k-atari-mint-}"
XCC="${XCC:-${XPREFIX}gcc}"
XAR="${XAR:-${XPREFIX}ar}"
XRANLIB="${XRANLIB:-${XPREFIX}ranlib}"
for t in "$XCC" "$XAR"; do
    [ -x "$t" ] || { echo "error: cross tool missing: $t" >&2
                     echo "       install the classic m68k-atari-mint GCC (not mintelf)" >&2
                     exit 1; }
done

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
tool_or_die "$XAR"

# --- extract the pristine gemlib source (no overlay) -------------------------
TARBALL="$GEM/gemlib-0.44.0.tar.xz"
if [ "${GEMLIB_REFRESH:-0}" = "1" ]; then rm -rf "$TOP"; fi
if [ ! -d "$TOP" ]; then
  [ -f "$TARBALL" ] || { echo "error: pristine tarball $TARBALL missing" >&2; exit 1; }
  echo "extracting pristine gemlib from $(basename "$TARBALL")…"
  tmp="$(mktemp -d)"
  tar xf "$TARBALL" -C "$tmp" || { echo "error: extract failed" >&2; exit 1; }
  mv "$tmp/gemlib-0.44.0" "$TOP"
  rmdir "$tmp" 2>/dev/null || true
fi

# --- generate mt_gem.h from mt_gem.h.in (as gemlib's own Makefile does) ------
if [ -f "$TOP/mt_gem.h.in" ]; then
  sed -e 's,@MAJOR@,0,g' -e 's,@MINOR@,44,g' -e 's,@REVISION@,0,g' -e 's,@BETATAG@,"",g' \
      "$TOP/mt_gem.h.in" > "$TOP/mt_gem.h"
fi

# --- flags -------------------------------------------------------------------
# gemlib's own CONFIGVARS build uses -I$(srcdir) -O2; the rest is the TT target.
CFLAGS=(-m68020-60 -O2 -fomit-frame-pointer -I"$TOP")

echo "gcc : $("$XCC" --version | head -1)"
echo "cpu : -m68020-60 (multilib $("$XCC" -m68020-60 -print-multi-directory), hardware FPU)"
echo "src : $TOP (pristine, NO NewTOS overlay)"
echo "out : $OUT"
echo

# --- authoritative file list: expand gemlib's own COBJS manifest -------------
COBJS_MK="$BUILD/print_cobjs.mk"
COBJS_LIST="$BUILD/cobjs.txt"
{ echo 'empty ='; echo 'include SRCFILES'; echo 'print:'; printf '\t@printf "%%s\\n" $(COBJS)\n'; } > "$COBJS_MK"
( cd "$TOP" && make -f "$COBJS_MK" print 2>/dev/null ) | grep -E '\.c$' | sort -u > "$COBJS_LIST"
n=$(wc -l < "$COBJS_LIST" | tr -d ' ')
[ "$n" -gt 0 ] || { echo "error: could not expand COBJS from SRCFILES" >&2; exit 1; }
echo "COBJS manifest: $n files"

# --- compile (best effort; the linker pulls only what a client needs) --------
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"
ok=0; fail=0
while IFS= read -r c; do
    [ -n "$c" ] || continue
    src="$TOP/$c"
    [ -e "$src" ] || { echo "$c (missing)" >> "$BUILD/failed.txt"; fail=$((fail+1)); continue; }
    b="${c%.c}"
    if "$XCC" "${CFLAGS[@]}" -c "$src" -o "$OBJ/$b.o" 2>"$OBJ/$b.err"; then
        echo "$c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$OBJ/$b.o"
    fi
done < "$COBJS_LIST"

rm -f "$OUT"
"$XAR" cr "$OUT" "$OBJ"/*.o || exit 1
"$XRANLIB" "$OUT" 2>/dev/null || true
echo "compiled OK=$ok  skipped=$fail  -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
[ "$fail" -gt 0 ] && { echo "skipped files (see $BUILD/failed.txt):"; sed 's/^/    /' "$BUILD/failed.txt"; }
exit 0
