#!/usr/bin/env bash
#
# build-libshare.sh -- build libshare 0.1.1 (Arnaud Bercegeay, 2004-2005;
# LICENCE UNCLEAR, see libshare-sources.sha256) into a NewTOS/AArch64 static
# library sdk/libshare/libshare.a: the allocator every LDG module (.NLD) is
# linked with (TASK/LDG-M3-B2, docs/analysis/ldg-module-malloc-2026-10-06.md
# option A).
#
# WHY MODULES NEED IT. A module is a mintlib program that LDG starts once with
# Pexec(4) and that then lives on inside its client. mintlib's malloc keeps a
# static free list and gets its memory with GEMDOS Malloc, i.e. owned by
# whoever runs at that moment. The module's own exit() closes stdio
# (fcloseall -> fclose -> __flshfp -> init_stream: malloc(BUFSIZ) + free), so
# the hunk is the MODULE's, its Pterm frees it -- EmuTOS free_all_owned,
# bdos/proc.c:92-101,136-140 -- and the module's free list goes on pointing
# into it. The next malloc inside the module (called by the client) reads
# freed memory: Bus Error under memprot=on, double use / hang under off
# (measurements/measurement-2026-10-06-ldg-b2/RUNINFO.TXT). libshare's malloc
# takes its FIRST block from a static 16 KiB array in the module's BSS
# (malloc.c:50-52,120-126), which lives in the TPA the client owns and
# survives the module's Pterm, and gets every further block through
# ldg_Malloc (sbrk.c:68-75), i.e. in the context of the caller -- the client
# once the module is loaded. That is what zView's codecs link it for
# (`LIB = -lshare -lldg`).
#
# SOURCE PIN: sdk/libshare/libshare-0.1.1.tgz, the original release, sha256
# in libshare-sources.sha256 (origin URL and the licence note there). The
# archive's prebuilt m68k libshare.a and m68k test binaries are deleted after
# extraction.
#
# What is compiled: libshare's own object list, makefile:28
#   OBJ = libshare.o gl_shm.o calloc.o chdir.o malloc.o realloc.o sbrk.o
# with its own flags, makefile:17-26 (-O2 -fomit-frame-pointer -Wall
# -Wmissing-prototypes -Winline -Wshadow -Wpointer-arith -Wcast-qual
# -Waggregate-return), against mintlib's headers and LDG's ldg.h (libshare's
# makefile relies on both being installed system headers).
#
# Overlays (sdk/libshare/overlay/, each file's header gives line and reason):
#   sbrk.c:61    sbrk(size_t) -> sbrk(intptr_t): mintlib 0.60.1's <unistd.h>
#   calloc.c:25  __bzero -> bzero: the SDK's libc has no __bzero
#   malloc.c:51  static_mblock aligned like struct mem_chunk (LP64)
# libshare.c, gl_shm.c, chdir.c, realloc.c, lib.h, libshare.h unmodified.
#
# ONE ADDITION, NOT FROM LIBSHARE: mintlib's own mintlib/posix_memalign.c
# (memalign, posix_memalign, aligned_alloc), compiled unmodified with
# -D__malloc=malloc into this archive. Reason: in mintlib 0.60.1 those
# functions call `__malloc` (posix_memalign.c:22,25), the strong name of
# mintlib's allocator, which libshare (written in 2004) does not replace;
# a module calling them would pull mintlib's malloc.o next to libshare's and
# run two allocators again. With this member first in the link, memalign &
# Co. -- and libc's valloc.c, which calls memalign -- use libshare's malloc.
# The block layout it relies on is identical (struct mem_chunk, VAL_ALLOC:
# libshare lib.h:16-27 = mintlib malloc_int.h:5-17, same source lineage), and
# libshare's free() ignores the VAL_ALLOC+1 marker it writes, as mintlib's
# does (libshare malloc.c:204, mintlib mintlib/malloc.c:158). The build checks that the module
# links contain none of mintlib's allocator members (libshare-link.sh).
#
# Usage:  sdk/libshare/build-libshare.sh   (LIBSHARE_REFRESH=1 forces a re-extract)
# Output: sdk/libshare/libshare.a; header for modules: -I sdk/libshare/libshare-0.1.1
#         (libshare.h). Link it FIRST in a module's archive group, before
#         libldg.a and libc.a (sdk/ldg/build-ldg-demo.sh, build-ldg-client.sh).
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBDIR="$SDK/libshare"
VER="0.1.1"
TOP="$LIBDIR/libshare-$VER"
BUILD="$LIBDIR/build"
OUT="$LIBDIR/libshare.a"
mkdir -p "$BUILD"

. "$SDK/lib-stale.sh"
. "$SDK/vendor-lib.sh"
# libc -> ldg -> libshare: compiled against mintlib's headers and ldg.h.
lib_ensure ldg || exit 1
MINTTOP="$SDK/libc/mintlib"
MINT="$MINTTOP/include"
LDGINC="$SDK/ldg/ldg-r133/include"
for d in "$MINT" "$LDGINC"; do
  [ -d "$d" ] || { echo "error: $d missing (library build did not extract it)" >&2; exit 1; }
done

vl_extract "$LIBDIR/libshare-$VER.tgz" "$LIBDIR/libshare-sources.sha256" "$TOP" "libshare-$VER" "${LIBSHARE_REFRESH:-0}" || exit 1
# Never ship the archive's m68k binaries: the prebuilt libshare.a (which
# carries the name of our output) and the test programs.
rm -f "$TOP/libshare.a" "$TOP/test/test.ldg" "$TOP/test/test.tos"
vl_overlay "$LIBDIR/overlay" "$TOP" || exit 1

vl_toolchain || exit 1

# Source list read from libshare's own makefile (OBJ = ...).
objs="$(sed -n 's/^OBJ[[:space:]]*=[[:space:]]*//p' "$TOP/makefile")"
[ -n "$objs" ] || { echo "error: could not read OBJ from $TOP/makefile" >&2; exit 1; }
SRCS=()
for o in $objs; do SRCS[${#SRCS[@]}]="${o%.o}.c"; done

LIB_CFLAGS=(-fomit-frame-pointer -Wall -Wmissing-prototypes -Winline -Wshadow
            -Wpointer-arith -Wcast-qual -Waggregate-return)
LIB_INCS=(-I"$TOP" -I"$LDGINC" -I"$MINT")
vl_compile "$TOP" "$BUILD/obj" "${SRCS[@]}" || exit 1

# The addition: mintlib's posix_memalign.c on libshare's malloc, compiled with
# build-libc.sh's own flags and include path (it is a libc-internal file).
PM="$MINTTOP/mintlib/posix_memalign.c"
[ -f "$PM" ] || { echo "error: $PM missing (run sdk/libc/build-libc.sh)" >&2; exit 1; }
if "$CLANG" "${VL_CFLAGS[@]}" -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE \
     -D__NO_LONG_DOUBLE_MATH -Wno-macro-redefined -D__malloc=malloc \
     -I"$MINTTOP" -I"$MINT" -I"$MINTTOP/mintlib" -I"$MINTTOP/stdlib" -isystem "$RES" \
     -c "$PM" -o "$BUILD/obj/mintlib_posix_memalign.o" 2>"$BUILD/obj/mintlib_posix_memalign.err"; then
  echo "compiled mintlib/posix_memalign.c with -D__malloc=malloc (addition, see header)"
  [ -s "$BUILD/obj/mintlib_posix_memalign.err" ] && sed 's/^/      /' "$BUILD/obj/mintlib_posix_memalign.err"
else
  echo "error: mintlib/posix_memalign.c did not compile:" >&2
  sed 's/^/    /' "$BUILD/obj/mintlib_posix_memalign.err" >&2; exit 1
fi
echo

vl_archive "$OUT" "$BUILD/obj" "$BUILD" || exit 1
echo "exported: $(tr '\n' ' ' < "$BUILD/defined.txt")"
