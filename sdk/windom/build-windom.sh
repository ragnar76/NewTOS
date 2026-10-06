#!/usr/bin/env bash
#
# build-windom.sh -- build WinDom 1.21.3 (Dominique Bereziat & Arnaud
# Bercegeay, LGPL 2.1), the GEM window library zView is written against, into
# a NewTOS/AArch64 static library sdk/windom/libwindom1.a (TASK/LDG-M3,
# docs/analysis/zview-ldg-2026-10-04.md P-WINDOM).
#
# SOURCE PIN: sdk/windom/windom-1.21.3.zip, the upstream SourceForge release,
# plus zView's patch sdk/windom/patches/zview-windom-1.21.3.patch (origin,
# hashes, license: windom-sources.sha256). zView's howto_build/howto.txt
# step 2-3: "compile & install windom 1.21.3 after applying the patch. Rename
# libwindom.a -> libwindom1.a, same for the header file (windom.h ->
# windom1.h)". This script does exactly that: the archive is libwindom1.a and
# the staged header directory sdk/windom/include/ holds windom1.h AND
# windom.h (the LDG demo client and other WinDom programs include
# <windom.h>), both the same patched file, plus WinDom's av.h and scancode.h.
#
# Every run re-extracts the pristine zip, applies the zView patch, then copies
# sdk/windom/overlay/ over the tree -- so the compiled tree is always exactly
# "zip + patch + overlay" (no stale overlay can survive its removal).
#
# What is compiled: WinDom's OWN object list for gcc, src/files.mak
#   WINDOMOBJ = $(KERNEL) $(EXTRAOBJ)      (gcc.mak: EXTRAOBJ is empty)
# with gcc.mak's flags: -O2 -fomit-frame-pointer, its warning set, -I../include
# -DMODIF_XEDIT. WinDom has no assembler sources.
#
# Overlays (sdk/windom/overlay/, mirroring the WinDom tree; each file's header
# says what and why with file:line) -- the LP64 corrections that headers and
# flags cannot make. The list and their kinds are in docs/sdk/12-windom.md.
#
# Flags: the SDK's freestanding AArch64 set (sdk/vendor-lib.sh VL_CFLAGS),
# -D__NEWTOS__ (gemlib's retargeted trap path), and -fsigned-char: WinDom was
# written for and tested with m68k compilers, whose plain char is SIGNED
# (Pure C, Sozobon, gcc m68k); AArch64's is unsigned. Keeping the original
# signedness keeps every char comparison as the authors ran it.
#
# Usage:  sdk/windom/build-windom.sh
# Output: sdk/windom/libwindom1.a; headers: -I sdk/windom/include
#         (compile programs with gemlib's and mintlib's include dirs too)
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/windom"
VER="1.21.3"
TOP="$LIBDIR/windom-$VER"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libwindom1.a"
INC="$LIBDIR/include"
SUMS="$LIBDIR/windom-sources.sha256"
PATCH="$LIBDIR/patches/zview-windom-$VER.patch"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
lib_ensure gemlib || exit 1
MINT="$SDK/libc/mintlib/include"
GEMTOP="$SDK/gemlib/gemlib-0.44.0"
for d in "$MINT" "$GEMTOP"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done

# --- pristine zip + zView patch + overlay, fresh every run -------------------
vl_extract "$LIBDIR/windom-$VER.zip" "$SUMS" "$TOP" "windom-$VER" 1 || exit 1
want="$(sed -e 's/#.*//' "$SUMS" | awk -v f="patches/$(basename "$PATCH")" '$2==f{print $1}')"
have="$(shasum -a 256 "$PATCH" | awk '{print $1}')"
if [ -z "$want" ] || [ "$want" != "$have" ]; then
  echo "error: $(basename "$PATCH") SHA-256 $have does not match $(basename "$SUMS") ($want)" >&2
  exit 1
fi
echo "applying zView's $(basename "$PATCH") (sha256 verified)…"
( cd "$TOP" && patch -p1 --no-backup-if-mismatch -s < "$PATCH" ) || { echo "error: zView patch did not apply" >&2; exit 1; }
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1

# --- staged headers ------------------------------------------------------------
rm -rf "$INC"; mkdir -p "$INC"
cp "$TOP/include/windom.h" "$INC/windom.h"
cp "$TOP/include/windom.h" "$INC/windom1.h"     # howto.txt step 3
cp "$TOP/include/av.h" "$TOP/include/scancode.h" "$INC/"

vl_toolchain || exit 1

# --- WinDom's own object list (src/files.mak KERNEL; EXTRAOBJ empty for gcc) --
objs="$(awk '/^KERNEL=/{f=1; sub(/^KERNEL=/,"")} f{print; if ($0 !~ /\\[[:space:]]*$/) exit}' "$TOP/src/files.mak" \
        | tr ' \t\\' '\n\n\n' | grep '\.o$')"
[ -n "$objs" ] || { echo "error: could not read KERNEL from $TOP/src/files.mak" >&2; exit 1; }
SRCS=()
for o in $objs; do SRCS[${#SRCS[@]}]="src/${o%.o}.c"; done
echo "WinDom sources (files.mak KERNEL): ${#SRCS[@]}"

# gcc.mak: WARN = -Wall -Wmissing-prototypes -Winline -Wshadow -Wpointer-arith
# -Wcast-qual -Waggregate-return; CFLAGS = -O2 -fomit-frame-pointer $(WARN)
# -I../include -DMODIF_XEDIT.
#
# -D__RSXHDR: src/globals.h:244-264 declares WinDom's own RSXHDR (the
# extended-RSC header); gemlib 0.44, newer than the gemlib 0.43 WinDom 1.21.3
# was written against (INSTALL), declares one too (mt_gem.h:2177-2205) behind
# the guard __RSXHDR, which exists for exactly this case. Defining the guard
# keeps WinDom's declaration and skips gemlib's. No WinDom source uses either
# (grep RSXHDR src/*.c: no hit), and windom.h -- what programs include -- does
# not include globals.h, so programs are unaffected.
#
# Three diagnostics that clang >= 15 turns into ERRORS are, as in the gcc of
# WinDom's and zView's time, kept as WARNINGS (still printed in the log):
#   -Wint-conversion           a pointer stored into a `long` field. This is
#        what zView's patch itself produces: it removes gcc-4-rejected
#        cast-as-lvalue forms and leaves plain `long = pointer` assignments
#        (objc_dup.c:136, objc_string.c:81 wp_spec = char *; popup.c:90
#        ub_parm = W_PARM *). On LP64 a `long` is 64 bits wide, so the value
#        is kept whole -- the field types (W_PARM.wp_spec, USERBLK.ub_parm)
#        are exactly the ones that must hold a pointer.
#   -Wimplicit-function-declaration  strcpy (vqt_xname2.c:45,
#        bubble_call.c:56) and strcmp (font_name2id.c:40) used without
#        <string.h>. The implicit `int f()` passes the pointer arguments
#        unchanged in x0/x1; strcpy's return value is not used at either
#        site and strcmp's is an int, so nothing is truncated.
#   -Wincompatible-function-pointer-types  cicon.c:204 stores
#        `int xdraw_cicon(PARMBLK *)` into USERBLK.ub_code (declared
#        `short (*)(PARMBLK *)`). The AES reads the WORD result from w0's
#        low half, which is the same register either way.
# -Wno-header-guard: one upstream typo, globals.h:29-30 (#ifndef
# __WINDOM_GLOBAL__ / #define __WINDOW_GLOBAL__), reported once per file.
LIB_CFLAGS=(-D__NEWTOS__ -D__RSXHDR -fsigned-char -fomit-frame-pointer -DMODIF_XEDIT
            -Wall -Wmissing-prototypes -Winline -Wshadow -Wpointer-arith
            -Wcast-qual -Waggregate-return
            -Wno-error=int-conversion -Wno-error=implicit-function-declaration
            -Wno-error=incompatible-function-pointer-types -Wno-header-guard)
LIB_INCS=(-I"$TOP/include" -I"$TOP/src" -I"$GEMTOP" -I"$MINT")
# Reproducible archive: globals.c:35-44 stores __DATE__/__TIME__ in the
# public `WinDom` version record (struct w_version). clang takes both from
# SOURCE_DATE_EPOCH; it is pinned to the release's own time stamp (the zip's
# top directory, 2004-11-24 22:09:16, read as UTC), so two builds are
# byte-identical and WinDom.date says when the sources were released.
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1101334156}"
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1
vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1
