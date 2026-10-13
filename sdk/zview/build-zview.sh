#!/usr/bin/env bash
#
# build-zview.sh -- build zView 1.0.1 (the LDG version of the z-tools image
# viewer, Zorro / Miro Kropacek) for NewTOS/AArch64 (TASK/ZVIEW-M4,
# docs/analysis/zview-ldg-2026-10-04.md P-ZVIEW and P-CODEC):
#
#   ZVIEW.NAP        the GEM program (zview/Makefile OBJ, WITHOUT the PDF
#                    module), linked like zview/Makefile:15,35 minus
#                    -lfreetype/-lpdf: WinDom (libwindom1.a), libldg.a,
#                    gemlib.a, libpng.a, libz.a, libm.a, libc.a
#   CODECS/*.NLD     ten of its eleven LDG codecs (zview/plugins/Makefile),
#                    each linked like its own Makefile ("-lshare -lldg" +
#                    its image library) with sdk/libshare/libshare.a first
#                    in the group; every link map is checked
#                    (sdk/libshare/libshare-link.sh: libshare's allocator
#                    only). tiff is NOT built: it needs libtiff + liblzma,
#                    which the SDK does not have (docs/sdk/13-zview.md).
#   ZVIEW.RSC        zview/zview.rsc converted by tools/mkrsc.py (the
#                    kernel's rsrc_load reads NRSC/LP64; WinDom's RsrcLoad
#                    calls it, docs/sdk/12-windom.md section 1)
#   ICONS/*.GOD      zview/_dist/icons -- the browser icons, read at run time
#                    through the GodPaint codec (catalog/catalog_icons.c:124-129)
#
# staged as build/dist/ZVIEW/ (the layout of the m68k distribution: the codec
# directory is "codecs" beside the program, plugins.c:56-60; zView finds it
# through its start directory, main.c:123-125).
#
# SOURCE PIN: sdk/zview/z-tools-04805b7.tar.xz (origin, git archive command,
# licence: zview-sources.sha256), extracted pristine to
# sdk/zview/z-tools-04805b7/ on first use or when the tarball is newer.
#
# ZVIEW AND THE CODECS STAY UNMODIFIED. The build compiles from a SHADOW
# TREE (build/tree = symlinks to the pristine files, the overlays from
# sdk/zview/overlay/ symlinked over them -- the build-teradesk.sh model), so
# a .c overlay replaces the pristine file for every translation unit. Each
# overlay's header names the upstream file:line it changes and why; the list
# with kinds is in docs/sdk/13-zview.md:
#   zview/plugins.c       .NLD instead of .ldg (ADR-018 addendum)
#   zview/main.c,winimg.c the two call sites into the PDF module removed
#   zview/av_prot.c       VA_START pointer read as the hi/lo WORD pair
#   zview/custom_font.c   draw_text glyph WORDs built endian-neutrally (ZVIEW01)
#   zview/plugins/{degas,neochrome,godpaint,eureka,img}/*.c
#                         big-endian file headers/palettes/pixel words
#   zview/plugins/img/xp32.c   AArch64 equivalent of the m68k xp32.S
#   zview/plugins/jpg/jpgldg.c FILE * in an int32 field; long cookie values
#   zview/plugins/jpg/jpgdsp.c m68k register-call macros -> C calls
# What flags and headers do instead of source edits:
#   -DHAVE_INTS_DEFINED -include sdk/zview/include/zview-ints.h
#       types2b.h's int32/uint32 are "long" (64-bit on LP64); the prelude pins
#       the 8/16/32-bit names to their m68k widths through types2b.h's own
#       HAVE_INTS_DEFINED hook (the header says why)
#   -fsigned-char
#       zView and the codecs were built with m68k-atari-mint-gcc, whose plain
#       char is signed (the Makefiles do not override it); AArch64's is
#       unsigned. Same reasoning as WinDom (sdk/windom/build-windom.sh)
#   -fgnu89-inline
#       zvdi/dither.c, raster.c, raster_resize.c, p2c.c define their
#       functions "inline" and other files call them (dither.h declares them
#       "extern inline"): GNU89 inline semantics, the default of the gcc that
#       built zView (C99 semantics emit no external definition)
#   -I<tree>/ztask
#       "gmem.h": zView's build takes it from xpdf's goo directory
#       (Makefile:12,15); z-tools' own ztask/gmem.{c,h} stand in (see
#       zview-sources.sha256), gmem.c is compiled with zView's flags
#   _stksize = 512 KiB in ZVIEW.NAP
#       zview/Makefile:9,40 "m68k-atari-mint-stack -S 512k" patches mintlib's
#       _stksize in the linked program; zv_set_stack below does the same to
#       the ELF's initialised .data. The codecs ask for 64k (their
#       Makefiles), which is mintlib's MINKEEP default -- nothing to patch.
#   upstream warning flags (-Wall -Wshadow -Wno-pointer-sign for zView,
#       -Wall -Wno-pointer-sign for the codecs, common.mak:30): their
#       warnings are left visible.
#
# Usage:  sdk/zview/build-zview.sh
# Output: sdk/zview/build/dist/ZVIEW/ and sdk/zview/build/samples/IMAGES/
#         (tools/make-sd-image.sh installs them as C:\ZVIEW\ and
#         C:\ZVIEW\SAMPLES\ -- QEMU image only)
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
ZD="$SDK/zview"
VER="04805b7"
TOP="$ZD/z-tools-$VER"
SUMS="$ZD/zview-sources.sha256"
OVL="$ZD/overlay"
PRELUDE="$ZD/include/zview-ints.h"
BUILD="$ZD/build"
TREE="$BUILD/tree"
OBJ="$BUILD/obj"
DIST="$BUILD/dist/ZVIEW"
LIBC="$SDK/libc"
GEM="$SDK/gemlib"
LIBM="$SDK/libm"
WD="$SDK/windom"
LDGDIR="$SDK/ldg"
GEMTOP="$GEM/gemlib-0.44.0"
LIBMTOP="$LIBM/openlibm-0.8.8"
ZVIEW_STACK=$((512 * 1024))
mkdir -p "$BUILD"

# --- libraries ---------------------------------------------------------------
# zView: libc -> gemlib -> windom, libc -> ldg, libc -> zlib -> libm -> libpng.
# Codecs: + libshare (libc -> ldg -> libshare), libjpeg, libexif, giflib.
. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
. "$SDK/libshare/libshare-link.sh"
for l in windom libm ldg libshare zlib libpng libjpeg libexif giflib; do
  lib_ensure "$l" || exit 1
done
MINT="$LIBC/mintlib/include"
ZLIBTOP="$(ls -d "$SDK"/zlib/zlib-*/ | head -1)"; ZLIBTOP="${ZLIBTOP%/}"
PNGTOP="$(ls -d "$SDK"/libpng/libpng-*/ | head -1)"; PNGTOP="${PNGTOP%/}"
JPEGTOP="$(ls -d "$SDK"/libjpeg/jpeg-*/ | head -1)"; JPEGTOP="${JPEGTOP%/}"
GIFTOP="$(ls -d "$SDK"/giflib/giflib-*/ | head -1)"; GIFTOP="${GIFTOP%/}"
EXIFTOP="$(ls -d "$SDK"/libexif/libexif-*/ | head -1)"; EXIFTOP="${EXIFTOP%/}"
SHARETOP="$SDK/libshare/libshare-0.1.1"
for d in "$MINT" "$GEMTOP" "$LIBMTOP" "$WD/include" "$LDGDIR/ldg-r133/include" \
         "$ZLIBTOP" "$PNGTOP" "$JPEGTOP" "$GIFTOP" "$EXIFTOP" "$SHARETOP"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done

# --- pristine source -----------------------------------------------------------
vl_extract "$ZD/z-tools-$VER.tar.xz" "$SUMS" "$TOP" "z-tools-$VER" "${ZVIEW_REFRESH:-0}" || exit 1

# --- toolchain -----------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="${LLD:-$BIN/rust-lld}"
NM="$BIN/llvm-nm"
READOBJ="$BIN/llvm-readobj"
for t in "$LLD" "$NM" "$READOBJ"; do
  [ -x "$t" ] || { echo "error: missing LLVM tool: $t" >&2; exit 1; }
done
echo "clang : $($CLANG --version | head -1)"
echo

# --- shadow tree: pristine files symlinked, overlays layered on top -----------
rm -rf "$TREE"; mkdir -p "$TREE"
( cd "$TOP" && find . -type d ) | while IFS= read -r d; do mkdir -p "$TREE/$d"; done
( cd "$TOP" && find . -type f ) | while IFS= read -r f; do ln -s "$TOP/${f#./}" "$TREE/${f#./}"; done
n_ovl=0
while IFS= read -r f; do
  f="${f#./}"
  mkdir -p "$TREE/$(dirname "$f")"
  ln -sf "$OVL/$f" "$TREE/$f"
  echo "  overlay: $f"
  n_ovl=$((n_ovl+1))
done < <(cd "$OVL" && find . -type f \( -name '*.c' -o -name '*.h' \) ! -name '._*' | sort)
echo "shadow tree: $TREE (overlays: $n_ovl)"
ZV="$TREE/zview"
PL="$ZV/plugins"

# --- flags -----------------------------------------------------------------------
BASE=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE
      -fno-builtin -nostdinc -fsigned-char -fgnu89-inline -D__NEWTOS__
      -DHAVE_INTS_DEFINED -include "$PRELUDE" -Wno-macro-redefined)
ZV_WARN=(-Wall -Wshadow -Wno-pointer-sign)          # zview/Makefile:16
CODEC_WARN=(-Wall -Wno-pointer-sign)                 # plugins/*/Makefile + common.mak:30
COMMON_INCS=(-I"$GEMTOP" -I"$MINT" -I"$LIBM/include" -I"$LIBMTOP/include" -isystem "$RES")
ZV_INCS=(-I"$TREE/common" -I"$ZV" -I"$TREE/ztask" -I"$WD/include"
         -I"$LDGDIR/ldg-r133/include" -I"$PNGTOP" -I"$ZLIBTOP" "${COMMON_INCS[@]}")
CODEC_INCS=(-I"$TREE/common" -I"$PL/common" -I"$SHARETOP" -I"$LDGDIR/ldg-r133/include"
            -I"$PNGTOP" -I"$ZLIBTOP" -I"$JPEGTOP" -I"$GIFTOP" -I"$EXIFTOP" "${COMMON_INCS[@]}")

rm -rf "$OBJ"; mkdir -p "$OBJ/zview" "$OBJ/codecs"
: > "$BUILD/warnings.txt"
cc() { # cc <zview|codec> <src> <obj>
  local kind=$1 src=$2 o=$3 e="${3%.o}.err"
  local flags=("${BASE[@]}")
  if [ "$kind" = zview ]; then flags+=("${ZV_WARN[@]}" "${ZV_INCS[@]}")
  else flags+=("${CODEC_WARN[@]}" "${CODEC_INCS[@]}"); fi
  if ! "$CLANG" "${flags[@]}" -c "$src" -o "$o" 2>"$e"; then
    echo "error: ${src#$TREE/} did not compile:" >&2; sed 's/^/    /' "$e" >&2; exit 1
  fi
  if [ -s "$e" ]; then
    echo "${src#$TREE/}: $(grep -c 'warning:' "$e") warning(s)" >> "$BUILD/warnings.txt"
    sed 's/^/    /' "$e" >> "$BUILD/warnings.txt"
  fi
}
mkprint() { printf 'print:\n\t@echo $(%s)\n' "$1" > "$BUILD/print-$1.mk"; echo "$BUILD/print-$1.mk"; }

# crt0.S: the app flag set of build-qed.sh (no C prelude in an assembler file).
"$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc \
    -D__NEWTOS__ -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$OBJ/crt0.o" || exit 1

# --- zView: its own OBJ list (zview/Makefile:23-29) minus the PDF module ------
# pdf_load.o and winpdf.o call into pdflib.cc (xpdf, C++, freetype) and are
# reached only through the two call sites the main.c/winimg.c overlays remove;
# pdflib.o itself is in Makefile's PDF_OBJ (libpdf.a), not in OBJ.
ZV_OBJS="$(make -s -C "$ZV" -f Makefile -f "$(mkprint OBJ)" print)"
n=0
for o in $ZV_OBJS; do
  case "$o" in pdf_load.o|winpdf.o) echo "  skip (PDF module): zview/${o%.o}.c"; continue;; esac
  cc zview "$ZV/${o%.o}.c" "$OBJ/zview/$(echo "${o%.o}" | tr '/' '_').o"; n=$((n+1))
done
cc zview "$TREE/ztask/gmem.c" "$OBJ/zview/ztask_gmem.o"; n=$((n+1))
echo "zView: compiled $n files (Makefile OBJ minus pdf_load.o winpdf.o, plus ztask/gmem.c)"

zv_link() { # zv_link <module|program> <out> <map> <archives...> -- <objs...>
  local kind=$1 out=$2 map=$3; shift 3
  local arch=() objs=()
  while [ "$1" != "--" ]; do arch+=("$1"); shift; done; shift
  objs=("$@")
  if ! "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$OBJ/crt0.o" "${objs[@]}" --start-group "${arch[@]}" --end-group \
      -Map="$map" -o "$out" 2>"$out.err"; then
    echo "error: link of $(basename "$out") failed:" >&2; sed 's/^/    /' "$out.err" >&2; exit 1
  fi
  local u; u=$(ls_undefined_count "$out")
  echo "LINK OK -> $(basename "$out") ($(wc -c < "$out" | tr -d ' ') bytes, $("$READOBJ" --file-headers "$out" | sed -n 's/^ *Type: //p'), undefined symbols: $u)"
  [ "$u" -eq 0 ] || { echo "error: $(basename "$out") has undefined symbols" >&2; exit 1; }
  if [ "$kind" = module ]; then
    ls_check_module_map "$map" "$(basename "$out")" || { rm -f "$out"; exit 1; }
  fi
}

# zv_set_stack <elf> <bytes>: the m68k-atari-mint-stack step. Writes <bytes>
# into mintlib's initialised `long _stksize` (sdk/libc/mintlib/mintlib/
# stksiz.c) inside the ELF file: symbol VA -> the PT_LOAD that holds it ->
# file offset. Fails if the symbol is missing or not file-backed.
zv_set_stack() {
  local elf=$1 bytes=$2 va
  va="$("$NM" "$elf" | awk '$3=="_stksize"{print $1}')"
  [ -n "$va" ] || { echo "error: _stksize not found in $elf" >&2; exit 1; }
  "$READOBJ" --program-headers "$elf" > "$BUILD/phdrs.txt"
  python3 - "$elf" "$va" "$bytes" "$BUILD/phdrs.txt" <<'PY' || exit 1
import re, struct, sys
elf, va, val, ph = sys.argv[1], int(sys.argv[2], 16), int(sys.argv[3]), open(sys.argv[4]).read()
for blk in ph.split("ProgramHeader {")[1:]:
    get = lambda k: int(re.search(r"\b%s: (0x[0-9A-Fa-f]+|\d+)" % k, blk).group(1), 0)
    if "PT_LOAD" not in blk:
        continue
    vaddr, off, filesz = get("VirtualAddress"), get("Offset"), get("FileSize")
    if vaddr <= va and va + 8 <= vaddr + filesz:
        with open(elf, "r+b") as f:
            f.seek(off + va - vaddr)
            old = struct.unpack("<q", f.read(8))[0]
            f.seek(off + va - vaddr)
            f.write(struct.pack("<q", val))
        print("  _stksize at 0x%x: %d -> %d" % (va, old, val))
        sys.exit(0)
sys.exit("error: _stksize (0x%x) is not in a file-backed PT_LOAD" % va)
PY
}

mkdir -p "$BUILD/out"
zv_link program "$BUILD/out/ZVIEW.NAP" "$BUILD/link-ZVIEW.NAP.map" \
  "$WD/libwindom1.a" "$LDGDIR/libldg.a" "$GEM/gemlib.a" "$SDK/libpng/libpng.a" "$SDK/zlib/libz.a" \
  "$LIBM/libm.a" "$LIBC/libc.a" -- "$OBJ"/zview/*.o
zv_set_stack "$BUILD/out/ZVIEW.NAP" "$ZVIEW_STACK"

# --- codecs: <dir> <output name> <image libraries> ---------------------------
# Object lists come from each codec's own Makefile (OBJ, plus DSP_OBJ for
# jpg); img's xp32.o is compiled from the overlay xp32.c (its xp32.S is m68k).
CODECS="bmp:BMP: degas:DEGAS: eureka:EUREKA: godpaint:GODPAINT: img:IMG: neochrome:NEO: tga:TGA:
        gif:GIF:giflib png:PNG:libpng,zlib jpg:JPG:libjpeg,libexif"
libpath() {
  case "$1" in
    giflib) echo "$SDK/giflib/libgif.a" ;; libpng) echo "$SDK/libpng/libpng.a" ;;
    zlib) echo "$SDK/zlib/libz.a" ;; libjpeg) echo "$SDK/libjpeg/libjpeg.a" ;;
    libexif) echo "$SDK/libexif/libexif.a" ;;
  esac
}
mkdir -p "$BUILD/out/CODECS"
nc=0
for spec in $CODECS; do
  IFS=: read -r dir name libs <<< "$spec"
  objs=()
  for o in $(make -s -C "$PL/$dir" -f Makefile -f "$(mkprint OBJ)" print); do
    src="$PL/$dir/${o%.o}.c"
    [ -e "$src" ] || { echo "error: $dir/$o has no C source in the shadow tree" >&2; exit 1; }
    cc codec "$src" "$OBJ/codecs/${dir}_${o}"
    objs+=("$OBJ/codecs/${dir}_${o}")
  done
  arch=("$SDK/libshare/libshare.a" "$LDGDIR/libldg.a")
  IFS=, read -r -a ls <<< "${libs:-}"
  for l in ${ls[@]+"${ls[@]}"}; do arch+=("$(libpath "$l")"); done
  arch+=("$GEM/gemlib.a" "$LIBM/libm.a" "$LIBC/libc.a")
  zv_link module "$BUILD/out/CODECS/$name.NLD" "$BUILD/link-$name.NLD.map" "${arch[@]}" -- "${objs[@]}"
  nc=$((nc+1))
done
echo "codecs: $nc built (tiff not built: libtiff + liblzma are not in the SDK)"

# --- dist ----------------------------------------------------------------------
rm -rf "$BUILD/dist"; mkdir -p "$DIST/CODECS" "$DIST/ICONS"
cp "$BUILD/out/ZVIEW.NAP" "$DIST/ZVIEW.NAP"
cp "$BUILD/out/CODECS/"*.NLD "$DIST/CODECS/"
python3 "$SDK/../tools/mkrsc.py" convert "$ZV/zview.rsc" "$DIST/ZVIEW.RSC" || exit 1
for f in "$ZV/_dist/icons/"*.god; do
  b="$(basename "$f" .god)"; cp "$f" "$DIST/ICONS/$(echo "$b" | tr '[:lower:]' '[:upper:]').GOD"
done
# sample images, one per codec path (samples/gen-samples.py, needs Python
# with Pillow): build/samples/IMAGES/ (installed by make-sd-image.sh as
# C:\ZVIEW\SAMPLES\) and build/samples/expected/ (screen references).
rm -rf "$BUILD/samples"
if python3 "$ZD/samples/gen-samples.py" "$BUILD/samples" > "$BUILD/samples.log" 2>&1; then
  echo "samples -> $BUILD/samples/IMAGES ($(ls "$BUILD/samples/IMAGES" | wc -l | tr -d ' ') files)"
else
  echo "note: samples/gen-samples.py failed (Pillow missing?), no sample images:"; sed 's/^/    /' "$BUILD/samples.log"
fi
echo "warnings: $(grep -c ': [0-9]* warning(s)$' "$BUILD/warnings.txt" || true) files (list: $BUILD/warnings.txt)"
echo "dist -> $DIST"
( cd "$BUILD/dist" && find ZVIEW -type f | sort | sed 's/^/    /' )
