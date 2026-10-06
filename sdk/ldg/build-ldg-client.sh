#!/usr/bin/env bash
#
# build-ldg-client.sh -- build LDG's WinDom-based demo client and its second
# demo module (TASK/LDG-M3, the M3 acceptance: "the unmodified demo/client
# runs against EX_LIB.NLD"):
#
#   LDGDEMO.NAP  <LDG>/demo/client/client.c (overlay: module suffix only, see
#                sdk/ldg/overlay/demo/client/client.c) + the demo's two
#                bindings <LDG>/demo/libs/exlibbnd.c and popupbnd.c,
#                unmodified, linked against WinDom (sdk/windom/libwindom1.a),
#                libldg.a, gemlib.a, libm.a, libc.a -- the link line of
#                demo/client/Makefile.mint (LDLIBS = -lldg -lwindom -lgem)
#   POPUP.NLD    <LDG>/demo/libs/popup.c, unmodified, the module the client
#                opens second (its __popup draws the "Functions" pop-up);
#                linked with sdk/libshare/libshare.a FIRST in its group
#                although popup.mint:10 links only -lgem -lldg (TASK/
#                LDG-M3-B2, a toolchain decision, the source is unchanged):
#                popup.c:74 calls malloc() inside a function the client
#                calls, which LDG's own rules forbid (guides/src/ldg.u:
#                1047-1051, guides/src/mem.stg:44-52: use ldg_Malloc &
#                Co.) -- with mintlib's malloc the
#                module's exit() leaves its free list pointing into a hunk
#                its own Pterm frees, and that malloc reads freed memory
#                (Bus Error under memprot=on, hang under off;
#                measurements/measurement-2026-10-06-ldg-b2). On the
#                reference it fails silently the same way. libshare's
#                malloc serves the exit-time buffer from a static block in
#                the module's BSS and later blocks via ldg_Malloc in the
#                caller's context. The link map is checked
#                (sdk/libshare/libshare-link.sh).
#   EX_LIB.NLD   built by build-ldg-demo.sh (M1), copied
#   client.rsc   <LDG>/demo/client/client.rsc converted by tools/mkrsc.py for
#                the kernel's rsrc_load (WinDom's RsrcLoad calls rsrc_load,
#                then reads rsh_ntree from the file's RSHDR -- the native
#                RSHDR preamble of the NRSC v13 image, 05-abi-pitfalls B8)
#
# The client opens its modules RELATIVE to its own directory
# ("..\libs\ex_lib.nld", client.c:173-174), and its resource by bare name, so
# the output is staged as the demo's own tree:
#
#   build/client/dist/LDGDEMO/CLIENT/LDGDEMO.NAP, CLIENT.RSC
#   build/client/dist/LDGDEMO/LIBS/EX_LIB.NLD, POPUP.NLD
#
# <LDG> = sdk/ldg/ldg-r133 (extracted + overlaid by build-ldg.sh). Flags: the
# app set of build-ldg-demo.sh. -fsigned-char is NOT used here: the client and
# popup.c are compiled like every other ported application; only WinDom keeps
# its m68k signedness inside the library (build-windom.sh).
#
# Usage:  sdk/ldg/build-ldg-client.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LDGDIR="$SDK/ldg"
TOP="$LDGDIR/ldg-r133"
LIBC="$SDK/libc"
GEM="$SDK/gemlib"
LIBM="$SDK/libm"
WD="$SDK/windom"
GEMTOP="$GEM/gemlib-0.44.0"
LIBMTOP="$LIBM/openlibm-0.8.8"
BUILD="$LDGDIR/build/client"
OBJ="$BUILD/obj"
DIST="$BUILD/dist/LDGDEMO"
mkdir -p "$OBJ"

# --- libraries: libc -> gemlib -> windom, libc -> libm, libc -> ldg -> libshare
. "$SDK/lib-stale.sh"
. "$SDK/libshare/libshare-link.sh"
lib_ensure windom   || exit 1
lib_ensure libm     || exit 1
lib_ensure ldg      || exit 1
lib_ensure libshare || exit 1
MINT="$LIBC/mintlib/include"
for d in "$MINT" "$GEMTOP" "$LIBMTOP" "$TOP" "$WD/include"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done
# EX_LIB.NLD is M1's module; its script also re-links LDGTEST.NCP.
"$LDGDIR/build-ldg-demo.sh" > "$BUILD/build-ldg-demo.log" 2>&1 || {
  echo "error: build-ldg-demo.sh failed:" >&2; sed 's/^/    /' "$BUILD/build-ldg-demo.log" >&2; exit 1; }

# --- toolchain ---------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
NM="$BIN/llvm-nm"
READOBJ="$BIN/llvm-readobj"
[ -x "$LLD" ] || { echo "error: missing LLVM tool: $LLD" >&2; exit 1; }

CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__
        -Wno-macro-redefined)
# WinDom's staged headers first (windom.h), then LDG's, the demo's module
# headers (<ldg/ex_lib.h>, <ldg/popup.h> live in demo/libs/), the client's own
# directory (client.h), gemlib, mintlib, OpenLibm.
INCS=(-I"$WD/include" -I"$TOP/include" -I"$TOP/demo/libs" -I"$TOP/demo/client"
      -I"$GEMTOP" -I"$MINT" -I"$LIBM/include" -I"$LIBMTOP/include" -isystem "$RES")

rm -f "$OBJ"/*.o "$OBJ"/*.err
cc() { # cc <src> <obj>
  if ! "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$1" -o "$OBJ/$2" 2>"$OBJ/${2%.o}.err"; then
    echo "error: $1 did not compile:" >&2; sed 's/^/    /' "$OBJ/${2%.o}.err" >&2; exit 1
  fi
  if [ -s "$OBJ/${2%.o}.err" ]; then
    echo "  warnings in $(basename "$1"):"; sed 's/^/      /' "$OBJ/${2%.o}.err"
  fi
}
[ -e "$LDGDIR/overlay/demo/client/client.c" ] && echo "  overlay: demo/client/client.c (from sdk/ldg/overlay/)"
cc "$TOP/demo/client/client.c"  client.o
cc "$TOP/demo/libs/exlibbnd.c"  exlibbnd.o
cc "$TOP/demo/libs/popupbnd.c"  popupbnd.o
cc "$TOP/demo/libs/popup.c"     popup.o
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$OBJ/crt0.o" || exit 1

link() { # link <module|client> <out> <objs...>
  local kind=$1 out=$2; shift 2
  local share=() map="$BUILD/link-$(basename "$out").map"
  # A module links libshare.a first in the group (see the header).
  [ "$kind" = module ] && share=("$SDK/libshare/libshare.a")
  if ! "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$OBJ/crt0.o" "$@" \
      --start-group ${share[@]+"${share[@]}"} "$WD/libwindom1.a" "$LDGDIR/libldg.a" "$GEM/gemlib.a" "$LIBM/libm.a" "$LIBC/libc.a" --end-group \
      -Map="$map" -o "$out" 2>"$BUILD/link-$(basename "$out").err"; then
    echo "error: link of $(basename "$out") failed:" >&2
    sed 's/^/    /' "$BUILD/link-$(basename "$out").err" >&2; exit 1
  fi
  echo "LINK OK -> $out ($(wc -c < "$out" | tr -d ' ') bytes)"
  "$READOBJ" --file-headers "$out" | grep -E '^ *Type:'
  echo "  undefined symbols (goal 0): $("$NM" -u "$out" 2>/dev/null | grep -c . || true)"
  if [ "$kind" = module ]; then
    ls_check_module_map "$map" "$(basename "$out")" || { rm -f "$out"; exit 1; }
  fi
}
link client "$BUILD/LDGDEMO.NAP" "$OBJ/client.o" "$OBJ/exlibbnd.o" "$OBJ/popupbnd.o"
link module "$BUILD/POPUP.NLD"   "$OBJ/popup.o"

# --- the demo's directory tree -----------------------------------------------
rm -rf "$BUILD/dist"; mkdir -p "$DIST/CLIENT" "$DIST/LIBS"
cp "$BUILD/LDGDEMO.NAP" "$DIST/CLIENT/LDGDEMO.NAP"
python3 "$SDK/../tools/mkrsc.py" convert "$TOP/demo/client/client.rsc" "$DIST/CLIENT/CLIENT.RSC" || exit 1
cp "$LDGDIR/build/EX_LIB.NLD" "$DIST/LIBS/EX_LIB.NLD"
cp "$BUILD/POPUP.NLD" "$DIST/LIBS/POPUP.NLD"
echo "dist -> $DIST"
( cd "$BUILD/dist" && find LDGDEMO -type f | sort | sed 's/^/    /' )
