#!/usr/bin/env bash
#
# build-cflib-ilp32.sh -- build cflib-21 into an **ILP32** NewTOS/AArch64
# cflib-ilp32.a and link the cftest cflib client into a native ELF32 program
# (.NCP). This is the GNU-ILP32 sibling of build-cflib.sh (ADR-020 Stage 3, the
# LP64->ILP32 app-ABI pivot). build-cflib.sh (clang/LP64) is left intact; this
# script and its artifacts are entirely separate:
#
#     tree     : sdk/cflib/cflib-21-ilp32/   (vs LP64 cflib-21/)
#     objects  : sdk/cflib/build-ilp32/      (vs LP64 build/)
#     archive  : sdk/cflib/cflib-ilp32.a     (vs LP64 cflib.a)
#
# cflib is a dialog/window/form helper built ON TOP of gemlib's AES/VDI bindings
# and mintlib's libc. It has NO own trap/asm boundary: it is pure C that calls
# appl_init/form_do/objc_draw/... from gemlib. The ILP32 port is therefore just
# "compile cflib against the already-ported gemlib-ilp32.a + libc-ilp32.a and fix
# only what the AArch64/ILP32 compiler rejects".
#
# Toolchain (GNU bare-metal AArch64, Homebrew aarch64-elf-*), identical to
# build-gemlib-ilp32.sh / build-libc-ilp32.sh:
#   - aarch64-elf-gcc -mabi=ilp32     -> 4-byte long/pointer, ELF32 objects.
#   - aarch64-elf-ar                  -> archive.
#   - aarch64-elf-ld -m aarch64elf32  -> ELF32 AArch64 static executable.
#
# Which LP64 overlays are DROPPED on ILP32+gcc: **none**. Both cflib overlays are
# arch-detection fixes gated on -D__NEWTOS__, not LP64/clang patches:
#   - fontsel.c  : gate the m68k xfsl_* inline-asm macros off under __NEWTOS__ so
#                  the direct-call (#else) branch is used (the selector entry
#                  points are ordinary __CDECL cookie function pointers, not a
#                  system trap). Arch boundary, ABI-agnostic.
#   - inline.rsh : extend the portab.h skip (was __GNUC__ && __mc68000__) to
#                  __NEWTOS__ too (types come from mt_gem.h; no <portab.h>).
# `aarch64-elf-gcc` defines `__GNUC__`, so both carry over verbatim, exactly like
# the LP64 build.
#
# The linker pulls only the archive members it needs, so cflib-ilp32.a is
# compiled from the whole COBJS manifest (SRCFILES) best-effort; a file that
# fails to compile is skipped and logged, and a *needed* one surfaces as an
# undefined symbol at link time.
#
# Usage:  sdk/cflib/build-cflib-ilp32.sh [client.c] [output.NCP]
#   client.c   defaults to sdk/cflib/tests/cftest.c
#   output.NCP defaults to <build-ilp32>/cftest.NCP
#
# Toolchain override:
#   GCC=/path/to/aarch64-elf-gcc LD=... AR=... sdk/cflib/build-cflib-ilp32.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
CF="$SDK/cflib"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
TOP="$CF/cflib-21-ilp32"                                     # ILP32 cflib top_srcdir
GEMTOP="$GEM/gemlib-0.44.0-ilp32"                            # ILP32 gemlib top_srcdir
CLIENT="${1:-$CF/tests/cftest.c}"
BUILD="$CF/build-ilp32"
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

# --- ensure the ILP32 mintlib libc + gemlib are present ---------------------
if [ ! -f "$LIBC/libc-ilp32.a" ]; then
  echo "libc-ilp32.a absent -- running build-libc-ilp32.sh first…"
  "$LIBC/build-libc-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-libc-ilp32.sh failed" >&2; exit 1; }
fi
MINT="$LIBC/mintlib-ilp32/include"
[ -d "$MINT" ] || { echo "error: ILP32 mintlib headers missing ($MINT); run build-libc-ilp32.sh" >&2; exit 1; }
if [ ! -f "$GEM/gemlib-ilp32.a" ]; then
  echo "gemlib-ilp32.a absent -- running build-gemlib-ilp32.sh first…"
  "$GEM/build-gemlib-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-gemlib-ilp32.sh failed" >&2; exit 1; }
fi
[ -d "$GEMTOP" ] || { echo "error: ILP32 gemlib headers missing ($GEMTOP); run build-gemlib-ilp32.sh" >&2; exit 1; }

# --- ensure the ILP32 cflib source tree is present (extract tarball + overlay) --
# The pristine cflib-21 source is vendored as a TARBALL; our NewTOS edits live in
# sdk/cflib/overlay/ (shared with the LP64 build; both overlays are __NEWTOS__
# arch fixes). The extracted ILP32 tree is a build artifact (gitignored, separate
# from the LP64 tree). CFLIB_REFRESH=1 forces a clean re-extract.
TARBALL="$CF/cflib-21.tar.xz"
OVERLAY="$CF/overlay"
if [ "${CFLIB_REFRESH:-0}" = "1" ]; then rm -rf "$TOP"; fi
if [ ! -d "$TOP" ]; then
  [ -f "$TARBALL" ] || { echo "error: pristine tarball $TARBALL missing" >&2; exit 1; }
  echo "extracting pristine cflib from $(basename "$TARBALL")…"
  tmp="$(mktemp -d)"
  tar xf "$TARBALL" -C "$tmp"
  mv "$tmp/cflib-21" "$TOP"
  rmdir "$tmp" 2>/dev/null || true
fi
# Always (re-)apply the NewTOS overlay so our retarget is present after any extract.
if [ -d "$OVERLAY" ]; then
  ( cd "$OVERLAY" && find . -type f -print0 | while IFS= read -r -d '' f; do
      mkdir -p "$TOP/$(dirname "$f")"
      cp "$f" "$TOP/$f"
    done )
fi

# --- flags (cflib -I$(top_srcdir) + -O2, retargeted for GNU ILP32) -----------
# cflib's own build uses -I$(top_srcdir) (its headers live in the top dir) + -O2.
# We add the freestanding/no-libc/-nostdinc set, the ILP32 gemlib + mintlib
# include paths, the gcc fixed-include dir, and -D__NEWTOS__ to select the
# overlay paths (fontsel.c direct call, inline.rsh no-portab).
GINC="$("$GCC" -mabi=ilp32 -print-file-name=include)"        # gcc fixed includes
CFLAGS=(-mabi=ilp32 -ffreestanding -nostdlib -O2
        -fno-pic -fno-builtin -nostdinc -D__NEWTOS__)
INCS=(-I"$TOP" -I"$GEMTOP" -I"$MINT" -isystem "$GINC")

echo "gcc : $("$GCC" -mabi=ilp32 --version 2>/dev/null | head -1)"
echo "ar  : $AR"
echo "ld  : $("$LD" --version | head -1)"
echo "abi : ILP32 (4-byte long/pointer, ELF32 AArch64)"
echo

# --- authoritative file list: expand cflib's own COBJS manifest --------------
COBJS_MK="$BUILD/print_cobjs.mk"
COBJS_LIST="$BUILD/cobjs.txt"
{ echo 'empty ='; echo 'include SRCFILES'; echo 'print:'; printf '\t@printf "%%s\\n" $(COBJS)\n'; } > "$COBJS_MK"
( cd "$TOP" && make -f "$COBJS_MK" print 2>/dev/null ) | grep -E '\.c$' | sort -u > "$COBJS_LIST"
n_cobjs=$(wc -l < "$COBJS_LIST" | tr -d ' ')
[ "$n_cobjs" -gt 0 ] || { echo "error: could not expand COBJS from SRCFILES" >&2; exit 1; }
echo "COBJS manifest: $n_cobjs files"

# --- compile cflib into objects (best-effort) --------------------------------
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
rm -f "$CF/cflib-ilp32.a"
"$AR" cr "$CF/cflib-ilp32.a" "$OBJ"/*.o
echo "compiled OK=$ok  skipped=$fail  -> $CF/cflib-ilp32.a ($(wc -c < "$CF/cflib-ilp32.a" | tr -d ' ') bytes)"
if [ "$fail" -ne 0 ]; then
    echo "skipped (did not compile for AArch64/ILP32):"
    sed 's/^/    /' "$BUILD/failed.txt"
fi
echo

# --- CRT + client ------------------------------------------------------------
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0-ilp32.S" -o "$BUILD/crt0.o" || exit 1
"$GCC" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$CLIENT" -o "$BUILD/client.o" 2>"$BUILD/client.err" \
    || { echo "client compile FAILED:"; cat "$BUILD/client.err"; exit 1; }

# --- link (crt0 + client + cflib-ilp32.a + gemlib-ilp32.a + libc-ilp32.a) ----
# cflib -> gemlib -> libc is the dependency order; wrap the three archives in a
# group so any residual back-reference between them still resolves.
"$LD" -m aarch64elf32 -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --no-warn-rwx-segments -Map "$BUILD/link.map" \
    "$BUILD/crt0.o" "$BUILD/client.o" \
    --start-group "$CF/cflib-ilp32.a" "$GEM/gemlib-ilp32.a" "$LIBC/libc-ilp32.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    [ -n "$READELF" ] && "$READELF" -h "$OUT" | grep -iE 'Class:|Machine:|Type:|Entry'
    if [ -n "$OBJDUMP" ]; then
        echo
        echo "GEM trap sites (svc #2) reachable from the linked cflib program:"
        "$OBJDUMP" -d "$OUT" | grep -iE '<aes>:|<vdi>:|svc[[:space:]]+#0x2' | sed 's/^/    /' | head
    fi
else
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined reference to \`[^']+'" "$BUILD/link.err" \
        | sed "s/.*\`//; s/'//" | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/link.err and $BUILD/link.map for detail."
    exit 1
fi
