#!/usr/bin/env bash
#
# build-gemlib-ilp32.sh -- build gemlib-0.44.0 into an **ILP32** NewTOS/AArch64
# gemlib-ilp32.a and link the gemtest GEM client into a native ELF32 program
# (.NCP). This is the GNU-ILP32 sibling of build-gemlib.sh (ADR-020 Stage 3, the
# LP64->ILP32 app-ABI pivot). build-gemlib.sh (clang/LP64) is left intact for the
# future LP64-native path; this script and its artifacts are entirely separate:
#
#     tree     : sdk/gemlib/gemlib-0.44.0-ilp32/   (vs LP64 gemlib-0.44.0/)
#     objects  : sdk/gemlib/build-ilp32/           (vs LP64 build/)
#     archive  : sdk/gemlib/gemlib-ilp32.a         (vs LP64 gemlib.a)
#
# Toolchain (GNU bare-metal AArch64, Homebrew aarch64-elf-*), identical to
# build-libc-ilp32.sh:
#   - aarch64-elf-gcc -mabi=ilp32     -> 4-byte long/pointer, ELF32 objects.
#     (GCC 16 prints "-mabi=ilp32 is deprecated"; it is the only spelling.)
#   - aarch64-elf-ar                  -> archive.
#   - aarch64-elf-ld -m aarch64elf32  -> ELF32 AArch64 static executable.
#
# gemlib's OS boundary is a single trap primitive per interface (AES `svc #2`
# selector 200, VDI `svc #2` selector 115); everything else marshals a parameter
# block and calls it. On ILP32 the app builds that block from **4-byte** pointers
# -- the whole point of this stage -- and the kernel's Aespb/Vdipb::from_raw read
# them at 4-byte stride (per-process ABI, tos-api). The library C is unchanged;
# only the register-svc primitives are retargeted, exactly as on LP64.
#
# Which LP64 overlays are DROPPED on ILP32+gcc: **none**. Every gemlib overlay is
# a `#ifdef __GNUC__` register-svc primitive (_gc_aes.c / _gc_vdi.c / _gc_vq_gdos.c
# / _gc_vq_vgdos.c / a__appl_yield.c) or the retargeted trap-macro header
# (gem_aesP.h / gem_vdiP.h routing through those primitives under -D__NEWTOS__).
# `aarch64-elf-gcc` defines `__GNUC__`, so all of them carry over verbatim -- the
# svc register ABI is ABI-agnostic (the block *contents* differ by pointer width,
# but the trap instruction and its x0/x1 register use do not). This mirrors
# build-libc-ilp32.sh, where the only LP64 overlay dropped (vfprintf.c) was a
# genuinely LP64-only source patch; gemlib has no such source-level overlay.
#
# The linker pulls only the archive members it needs, so gemlib-ilp32.a is
# compiled from the whole COBJS manifest best-effort (a file that fails to
# compile for AArch64/ILP32 is skipped and logged; a *needed* one surfaces as an
# undefined symbol at link time).
#
# Usage:  sdk/gemlib/build-gemlib-ilp32.sh [client.c] [output.NCP]
#   client.c   defaults to sdk/gemlib/tests/gemtest.c
#   output.NCP defaults to <build-ilp32>/gemtest.NCP
#
# Toolchain override:
#   GCC=/path/to/aarch64-elf-gcc LD=... AR=... sdk/gemlib/build-gemlib-ilp32.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
TOP="$GEM/gemlib-0.44.0-ilp32"                               # ILP32 gemlib top_srcdir
CLIENT="${1:-$GEM/tests/gemtest.c}"
BUILD="$GEM/build-ilp32"
OUT="${2:-$BUILD/$(basename "${CLIENT%.c}").NCP}"
OBJ="$BUILD/obj"
mkdir -p "$OBJ"

# --- locate the GNU bare-metal AArch64 toolchain ------------------------------
find_tool() { # $1 = env override (maybe empty), $2 = tool basename
    local override="$1" name="$2"
    if [ -n "$override" ]; then echo "$override"; return 0; fi
    if command -v "$name" >/dev/null 2>&1; then echo "$name"; return 0; fi
    if [ -x "/opt/homebrew/bin/$name" ]; then echo "/opt/homebrew/bin/$name"; return 0; fi
    return 1
}
GCC="$(find_tool "${GCC:-}" aarch64-elf-gcc)" \
    || { echo "error: aarch64-elf-gcc not found (brew install aarch64-elf-gcc)" >&2; exit 1; }
LD="$(find_tool "${LD:-}"  aarch64-elf-ld)" \
    || { echo "error: aarch64-elf-ld not found (brew install aarch64-elf-binutils)" >&2; exit 1; }
AR="$(find_tool "${AR:-}"  aarch64-elf-ar)" \
    || { echo "error: aarch64-elf-ar not found" >&2; exit 1; }
READELF="$(find_tool "${READELF:-}" aarch64-elf-readelf)" || READELF=""
OBJDUMP="$(find_tool "${OBJDUMP:-}" aarch64-elf-objdump)" || OBJDUMP=""

# --- ensure the ILP32 mintlib libc is present -------------------------------
# gemlib is compiled/linked against the already-ported ILP32 mintlib (headers +
# libc-ilp32.a), NOT the LP64 libc.a.
if [ ! -f "$LIBC/libc-ilp32.a" ]; then
  echo "libc-ilp32.a absent -- running build-libc-ilp32.sh first…"
  "$LIBC/build-libc-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-libc-ilp32.sh failed" >&2; exit 1; }
fi
MINT="$LIBC/mintlib-ilp32/include"
[ -d "$MINT" ] || { echo "error: ILP32 mintlib headers missing ($MINT); run build-libc-ilp32.sh" >&2; exit 1; }

# --- ensure the ILP32 gemlib source tree is present (extract tarball + overlay) --
# The pristine gemlib-0.44.0 source is vendored as a TARBALL; our NewTOS edits
# live in sdk/gemlib/overlay/ (shared with the LP64 build; all overlays are
# __GNUC__ register-svc). The extracted ILP32 tree is a build artifact
# (gitignored, separate from the LP64 tree). GEMLIB_REFRESH=1 forces a clean
# re-extract from the pristine tarball.
TARBALL="$GEM/gemlib-0.44.0.tar.xz"
OVERLAY="$GEM/overlay"
if [ "${GEMLIB_REFRESH:-0}" = "1" ]; then rm -rf "$TOP"; fi
if [ ! -d "$TOP" ]; then
  [ -f "$TARBALL" ] || { echo "error: pristine tarball $TARBALL missing" >&2; exit 1; }
  echo "extracting pristine gemlib from $(basename "$TARBALL")…"
  tmp="$(mktemp -d)"
  tar xf "$TARBALL" -C "$tmp"
  mv "$tmp/gemlib-0.44.0" "$TOP"
  rmdir "$tmp" 2>/dev/null || true
fi
# Always (re-)apply the NewTOS overlay so our retarget is present after any extract.
if [ -d "$OVERLAY" ]; then
  ( cd "$OVERLAY" && find . -type f -print0 | while IFS= read -r -d '' f; do
      mkdir -p "$TOP/$(dirname "$f")"
      cp "$f" "$TOP/$f"
    done )
fi

# --- generate mt_gem.h from mt_gem.h.in (as gemlib's Makefile does) ----------
# VERSION=0.44.0 -> MAJOR=0 MINOR=44 REVISION=0 BETATAG="" (no beta suffix).
if [ -f "$TOP/mt_gem.h.in" ]; then
  sed -e 's,@MAJOR@,0,g' -e 's,@MINOR@,44,g' -e 's,@REVISION@,0,g' -e 's,@BETATAG@,"",g' \
      "$TOP/mt_gem.h.in" > "$TOP/mt_gem.h"
fi

# --- flags (gemlib CONFIGVARS -I$(srcdir) + -O2, retargeted for GNU ILP32) ----
# gemlib's own build uses -I$(srcdir) (all headers live in the top dir) and -O2.
# We add the freestanding/no-libc/-nostdinc set + the ILP32 mintlib include path
# + the gcc fixed-include dir (-isystem), and -D__NEWTOS__ to select the
# retargeted trap path. Same shape as build-libc-ilp32.sh CFLAGS.
GINC="$("$GCC" -mabi=ilp32 -print-file-name=include)"        # gcc fixed includes
CFLAGS=(-mabi=ilp32 -ffreestanding -nostdlib -O2
        -fno-pic -fno-builtin -nostdinc -D__NEWTOS__)
INCS=(-I"$TOP" -I"$MINT" -isystem "$GINC")

echo "gcc : $("$GCC" -mabi=ilp32 --version 2>/dev/null | head -1)"
echo "ar  : $AR"
echo "ld  : $("$LD" --version | head -1)"
echo "abi : ILP32 (4-byte long/pointer, ELF32 AArch64)"
echo

# --- authoritative file list: expand gemlib's own COBJS manifest -------------
# SRCFILES defines COBJS from sub-variables (_VDI_CONTRL, _AES_*, ...). Expand it
# with make (SRCFILES has no rules needing the cross compiler). doxydoc.c is the
# only top-level .c NOT in COBJS (documentation) and is excluded automatically.
COBJS_MK="$BUILD/print_cobjs.mk"
COBJS_LIST="$BUILD/cobjs.txt"
{ echo 'empty ='; echo 'include SRCFILES'; echo 'print:'; printf '\t@printf "%%s\\n" $(COBJS)\n'; } > "$COBJS_MK"
( cd "$TOP" && make -f "$COBJS_MK" print 2>/dev/null ) | grep -E '\.c$' | sort -u > "$COBJS_LIST"
n_cobjs=$(wc -l < "$COBJS_LIST" | tr -d ' ')
[ "$n_cobjs" -gt 0 ] || { echo "error: could not expand COBJS from SRCFILES" >&2; exit 1; }
echo "COBJS manifest: $n_cobjs files"

# --- compile gemlib into objects (best-effort) -------------------------------
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"
ok=0; fail=0
while IFS= read -r c; do
    [ -n "$c" ] || continue
    src="$TOP/$c"
    [ -e "$src" ] || { echo "$c (missing)" >> "$BUILD/failed.txt"; fail=$((fail+1)); continue; }
    b="${c%.c}"
    o="$OBJ/${b}.o"
    if "$GCC" "${CFLAGS[@]}" "${INCS[@]}" -c "$src" -o "$o" 2>"$OBJ/${b}.err"; then
        echo "$c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
    fi
done < "$COBJS_LIST"

# --- archive -----------------------------------------------------------------
rm -f "$GEM/gemlib-ilp32.a"
"$AR" cr "$GEM/gemlib-ilp32.a" "$OBJ"/*.o
echo "compiled OK=$ok  skipped=$fail  -> $GEM/gemlib-ilp32.a ($(wc -c < "$GEM/gemlib-ilp32.a" | tr -d ' ') bytes)"
echo

# --- CRT + client ------------------------------------------------------------
# Reuse the ILP32 libc-aware crt0 (sets SP from p_hitpa, inits the malloc arena,
# builds { "", NULL } argv, calls main) and the NewTOS TPA linker script.
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0-ilp32.S" -o "$BUILD/crt0.o" || exit 1
"$GCC" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$CLIENT" -o "$BUILD/client.o" 2>"$BUILD/client.err" \
    || { echo "client compile FAILED:"; cat "$BUILD/client.err"; exit 1; }

# --- link (crt0 + client + gemlib-ilp32.a + libc-ilp32.a) --------------------
# `-m aarch64elf32` selects the ELF32 AArch64 output format from newtos.ld; the
# single RWX PT_LOAD is intentional (loader maps every TPA page UserRwExec).
"$LD" -m aarch64elf32 -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --no-warn-rwx-segments -Map "$BUILD/link.map" \
    "$BUILD/crt0.o" "$BUILD/client.o" "$GEM/gemlib-ilp32.a" "$LIBC/libc-ilp32.a" \
    -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    [ -n "$READELF" ] && "$READELF" -h "$OUT" | grep -iE 'Class:|Machine:|Type:|Entry'
    if [ -n "$OBJDUMP" ]; then
        echo
        echo "GEM trap sites (svc #2) reachable from the linked program:"
        "$OBJDUMP" -d "$OUT" | grep -iE '<aes>:|<vdi>:|svc[[:space:]]+#0x2' | sed 's/^/    /'
    fi
else
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined reference to \`[^']+'" "$BUILD/link.err" \
        | sed "s/.*\`//; s/'//" | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/link.err and $BUILD/link.map for detail."
    exit 1
fi
