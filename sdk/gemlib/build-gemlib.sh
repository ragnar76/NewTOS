#!/usr/bin/env bash
#
# build-gemlib.sh -- build gemlib-0.44.0 into a NewTOS/AArch64 gemlib.a and
# link the gemtest GEM client into a native program (.NCP).
#
# gemlib is the GEM (AES+VDI) binding library. Its OS boundary is a single trap
# primitive per interface; everything else marshals a parameter block and calls
# that primitive. We retarget ONLY the primitive (the m68k `trap #2` -> NewTOS
# `svc #2`) and compile the ~578 bindings unchanged with clang, exactly like the
# mintlib port (docs/MEMORY.md ADR-019, sdk/libc/build-libc.sh).
#
# Verified NewTOS GEM ABI (kernel/src/arch/aarch64/exceptions.rs handle_svc,
# GEM branch; tos-api/src/{vdi,aes}/mod.rs):
#   GEM trap  = `svc #2`
#   x0        = selector (115 = VDI, 200 = AES)
#   x1        = pointer to the classic parameter block:
#               VDI: 5 native pointers (contrl,intin,ptsin,intout,ptsout)
#               AES: 6 native pointers (control,global,intin,intout,addrin,addrout)
#   result    : none in a register (handler leaves x0 = 0); results come back
#               through the parameter-block arrays.
#
# NewTOS-specific deltas from the pristine m68k build:
#   - -D__NEWTOS__ selects the retargeted C-call trap path in the overlay
#     headers gem_aesP.h / gem_vdiP.h (AES_TRAP -> aes(), VDI_TRAP* -> vdi()),
#     bypassing the m68k inline asm. The C primitives themselves live in the
#     overlay _gc_aes.c / _gc_vdi.c / _gc_vq_gdos.c / _gc_vq_vgdos.c.
#   - The three m68k .s primitive files (_lc_asm.s/_pc_asm.s/_sx_asm.s) are
#     memory-model variants of the SAME aes/vdi/vq_gdos/vq_vgdos primitives for
#     non-GNU compilers; SOBJS is empty in the gcc build (SRCFILES), so they are
#     not compiled. The retargeted _gc_*.c C files are their reimplementation.
#   - mt_gem.h is GENERATED from mt_gem.h.in (as gemlib's Makefile does).
#
# Edits to vendored files live in sdk/gemlib/overlay/ (applied over the freshly
# extracted tree by the preamble below), mirroring sdk/libc/overlay/.
#
# The linker pulls only the archive members it needs, so gemlib.a is compiled
# from the whole COBJS manifest best-effort (files that fail to compile for
# AArch64/LP64 are skipped and logged; a *needed* one surfaces as an undefined
# symbol at link time).
#
# Usage:  sdk/gemlib/build-gemlib.sh [client.c] [output.NCP]
#   client.c   defaults to sdk/gemlib/tests/gemtest.c
#   output.NCP defaults to <builddir>/gemtest.NCP
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
TOP="$GEM/gemlib-0.44.0"                                     # gemlib top_srcdir
CLIENT="${1:-$GEM/tests/gemtest.c}"
BUILD="$GEM/build"
OUT="${2:-$BUILD/$(basename "${CLIENT%.c}").NCP}"
OBJ="$BUILD/obj"
mkdir -p "$OBJ"

# --- ensure the ported mintlib libc is present ------------------------------
# gemlib is compiled/linked against the already-ported mintlib (headers + libc.a).
# Rebuild a library when its archive is missing OR out of date. What "out of
# date" means for each library, and the fact that gemlib is built against
# mintlib's headers and must follow libc.a, is declared once in sdk/lib-stale.sh.
. "$SDK/lib-stale.sh"
lib_ensure libc || exit 1
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ] || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }

# --- ensure the gemlib source tree is present (extract tarball + overlay) ----
# The pristine gemlib-0.44.0 source is vendored as a TARBALL; our NewTOS edits
# live in sdk/gemlib/overlay/ (mirroring the gemlib layout). The extracted
# sdk/gemlib/gemlib-0.44.0/ tree is a build artifact (gitignored). Set
# GEMLIB_REFRESH=1 to force a clean re-extract from the pristine tarball.
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

# --- toolchain ---------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
AR="${AR:-$BIN/llvm-ar}"
LLD="${LLD:-$BIN/rust-lld}"
READOBJ="$BIN/llvm-readobj"
OBJDUMP="$BIN/llvm-objdump"
for t in "$AR" "$LLD"; do
    [ -x "$t" ] || { echo "error: missing LLVM tool: $t" >&2; exit 1; }
done

# --- flags (gemlib CONFIGVARS -I$(srcdir) + -O2, retargeted per the task) ----
# gemlib's own build uses -I$(srcdir) (all headers live in the top dir) and
# -O2. We add the freestanding/no-libc/-nostdinc set and the ported mintlib
# include path, plus -D__NEWTOS__ to select the retargeted trap path.
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc -D__NEWTOS__
        -Wno-macro-redefined)
INCS=(-I"$TOP" -I"$MINT" -isystem "$RES")

echo "clang : $($CLANG --version | head -1)"
echo "ar    : $AR"
echo "lld   : $LLD"
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
: > "$BUILD/failed-err.txt"   # "<source><TAB><its .err>" -- lib_check_failures
                              # quotes the diagnostic of an UNRECORDED failure
ok=0; fail=0
while IFS= read -r c; do
    [ -n "$c" ] || continue
    src="$TOP/$c"
    [ -e "$src" ] || { echo "$c (missing)" >> "$BUILD/failed.txt"; fail=$((fail+1)); continue; }
    b="${c%.c}"
    o="$OBJ/${b}.o"
    if "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$src" -o "$o" 2>"$OBJ/${b}.err"; then
        echo "$c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
        printf '%s\t%s\n' "$c" "$OBJ/${b}.err" >> "$BUILD/failed-err.txt"
    fi
done < "$COBJS_LIST"

# --- the failure list is REPORTED, and measured against the record -----------
# This script printed a count and no list (TASK/T02 named build-libc.sh; this
# one had the same hole, latent because gemlib's failure set is empty). The
# record IS empty, so any failure at all stops the build -- before the archive
# is written, so a silently incomplete gemlib.a is never left for the next run.
echo "compiled OK=$ok  skipped=$fail  (of $n_cobjs)"
lib_check_failures "gemlib" "$BUILD/failed.txt" "$GEM/expected-failures.txt" \
                   "$BUILD/failed-err.txt" || exit 1

# --- archive -----------------------------------------------------------------
rm -f "$GEM/gemlib.a"
"$AR" cr "$GEM/gemlib.a" "$OBJ"/*.o || exit 1
echo "archived -> $GEM/gemlib.a ($(wc -c < "$GEM/gemlib.a" | tr -d ' ') bytes)"
echo

# --- CRT + client ------------------------------------------------------------
# Reuse the libc-aware crt0 (sets SP from p_hitpa, inits the malloc arena, calls
# main) and the NewTOS TPA linker script.
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$CLIENT" -o "$BUILD/client.o" 2>"$BUILD/client.err" \
    || { echo "client compile FAILED:"; cat "$BUILD/client.err"; exit 1; }

# --- link (crt0 + client + gemlib.a + libc.a via the NewTOS TPA linker script) ---
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why.txt" \
    "$BUILD/crt0.o" "$BUILD/client.o" "$GEM/gemlib.a" "$LIBC/libc.a" -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    "$READOBJ" --file-headers "$OUT" | grep -iE 'Type:|Machine:|Entry:'
    echo
    echo "GEM trap sites (svc #2) reachable from the linked program:"
    "$OBJDUMP" -d "$OUT" | grep -iE '<aes>:|<vdi>:|svc[[:space:]]+#0x2' | sed 's/^/    /'
else
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link.err" | awk '{print $3}' | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/why.txt for the archive pull chains that reference them."
    exit 1
fi
