#!/usr/bin/env bash
#
# build-libm.sh -- build OpenLibm 0.8.8 into a NewTOS/AArch64 libm.a and link
# the mathtst client into a native program (.NCP).
#
# mintlib ships no libm and no <math.h> (on m68k both came from the compiler's
# PML/fdlibm; its include/tcmath.h even says "replace this file with math.h
# from the Pure/Turbo C distribution!"). OpenLibm -- the FreeBSD msun / fdlibm
# derived, standalone, MIT/BSD/ISC-licensed libm the Julia project maintains --
# is the SDK's libm (TASK/T07, lead decision 2026-09-21; provenance and license
# in sdk/libm/openlibm-sources.sha256 and LICENSE.openlibm.md). It is built the
# way the three other libraries are: pristine tarball + overlay/ + this script.
#
# What is compiled (OpenLibm's own manifests, sdk/libm/openlibm-0.8.8/*/Make.files,
# expanded by `make` exactly as OpenLibm's Makefile does it):
#   - src/      the C99 double/float functions and the complex functions
#   - bsdsrc/   b_exp.c b_log.c b_tgamma.c (tgamma)
#   - aarch64/  fenv.c (the <fenv.h> functions over FPCR/FPSR)
# and NOT:
#   - the LONG_DOUBLE_NOT_DOUBLE block of src/Make.files and all of ld128/:
#     AAPCS64 `long double` is IEEE binary128, clang lowers every operation on
#     it to a soft-float libcall (__addtf3, __multf3, ...), and the SDK links
#     no compiler-rt (docs/sdk/05-abi-pitfalls.md B12). The `l` functions are
#     therefore not built; they are a separate item (TASK/T07 "Arbeit" 2).
#     OpenLibm's own Darwin/aarch64 configuration makes the same cut
#     (Make.inc: LONG_DOUBLE_NOT_DOUBLE unset there), which is why no source
#     needs an overlay for it.
#   - the amd64/i387/arm/... directories: other architectures. OpenLibm has no
#     AArch64 assembly beyond fenv.c, so this is a pure C build.
#
# Flags: the shared SDK freestanding set + OpenLibm's own Make.inc CFLAGS_add
# for a clang build (-std=c99 -fno-builtin -fno-strict-aliasing
# -D__BSD_VISIBLE). Not taken from Make.inc: -fPIC (the SDK is -fPIE),
# -DASSEMBLER (only read by i387/osx_asm.h), -Wall (warnings go to the per-file
# .err files either way), -I ld128 (excluded above).
#
# Include order: OpenLibm's four (top, include/, aarch64/, src/), then mintlib's
# include/ for <assert.h> <ctype.h> <string.h> <stdint.h> <limits.h>, then
# clang's freestanding headers for <float.h>. The three libc symbols libm.a
# references -- __assert_fail, _ctype, memset -- resolve from libc.a, which is
# why the client link (and every app link) groups libm.a with libc.a.
#
# Headers for applications: OpenLibm names its headers openlibm_math.h /
# openlibm_complex.h / openlibm_fenv.h; a TOS program includes <math.h>. The
# adapters in sdk/libm/include/ map the standard names onto them and translate
# mintlib's feature macros into the __BSD_VISIBLE/__XSI_VISIBLE gates
# OpenLibm's headers use (M_PI & Co.). App scripts add
#   -I"$SDK/libm/include" -I"$SDK/libm/openlibm-0.8.8/include"
# after the mintlib include directory.
#
# Usage:  sdk/libm/build-libm.sh [client.c] [output.NCP]
#   client.c   defaults to sdk/examples/mathtst.c
#   output.NCP defaults to <builddir>/mathtst.NCP
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBM="$SDK/libm"
LIBC="$SDK/libc"
VER="0.8.8"
TOP="$LIBM/openlibm-$VER"                                    # openlibm top dir
CLIENT="${1:-$SDK/examples/mathtst.c}"
BUILD="$LIBM/build"
OUT="${2:-$BUILD/$(basename "${CLIENT%.c}").NCP}"
OBJ="$BUILD/obj"
mkdir -p "$OBJ"

# --- ensure the ported mintlib libc is present ------------------------------
# libm.a is compiled against mintlib's headers and linked with libc.a. The
# chain (libc -> libm) and what "out of date" means live in sdk/lib-stale.sh.
. "$SDK/lib-stale.sh"
lib_ensure libc || exit 1
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ] || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }

# --- ensure the OpenLibm source tree is present (verify + extract + overlay) --
# The pristine openlibm-0.8.8 source is vendored as a TARBALL, pinned by SHA-256
# in openlibm-sources.sha256; NewTOS edits (none so far) would live in
# sdk/libm/overlay/ mirroring the OpenLibm layout. The extracted tree is a
# build artifact (gitignored). OPENLIBM_REFRESH=1 forces a clean re-extract.
TARBALL="$LIBM/openlibm-$VER.tar.gz"
SUMS="$LIBM/openlibm-sources.sha256"
OVERLAY="$LIBM/overlay"
if [ "${OPENLIBM_REFRESH:-0}" = "1" ]; then rm -rf "$TOP"; fi
if [ ! -d "$TOP" ]; then
  [ -f "$TARBALL" ] || { echo "error: pristine tarball $TARBALL missing" >&2; exit 1; }
  want="$(sed -e 's/#.*//' "$SUMS" | awk -v f="openlibm-$VER.tar.gz" '$2==f{print $1}')"
  have="$(shasum -a 256 "$TARBALL" | awk '{print $1}')"
  if [ -z "$want" ] || [ "$want" != "$have" ]; then
    echo "error: $(basename "$TARBALL") SHA-256 $have does not match $(basename "$SUMS") ($want)" >&2
    exit 1
  fi
  echo "extracting pristine OpenLibm from $(basename "$TARBALL") (sha256 verified)…"
  tmp="$(mktemp -d)"
  tar xzf "$TARBALL" -C "$tmp"
  mv "$tmp/openlibm-$VER" "$TOP"
  rmdir "$tmp" 2>/dev/null || true
fi
if [ -d "$OVERLAY" ]; then
  ( cd "$OVERLAY" && find . -type f -print0 | while IFS= read -r -d '' f; do
      mkdir -p "$TOP/$(dirname "$f")"
      cp "$f" "$TOP/$f"
    done )
fi

# --- toolchain ---------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
command -v make >/dev/null 2>&1 || { echo "error: make not found (needed to expand OpenLibm's Make.files)" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
AR="${AR:-$BIN/llvm-ar}"
LLD="${LLD:-$BIN/rust-lld}"
READOBJ="$BIN/llvm-readobj"
NM="$BIN/llvm-nm"
for t in "$AR" "$LLD"; do
    [ -x "$t" ] || { echo "error: missing LLVM tool: $t" >&2; exit 1; }
done

# --- flags -------------------------------------------------------------------
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc
        -std=c99 -fno-strict-aliasing -D__BSD_VISIBLE)
INCS=(-I"$TOP" -I"$TOP/include" -I"$TOP/aarch64" -I"$TOP/src" -I"$MINT" -isystem "$RES")

echo "clang : $($CLANG --version | head -1)"
echo "ar    : $AR"
echo "lld   : $LLD"
echo

# --- authoritative file list: OpenLibm's own Make.files manifests ------------
# OpenLibm's Makefile includes <dir>/Make.files for SUBDIRS = src $(ARCH) bsdsrc
# (+ ld128 only when LONG_DOUBLE_NOT_DOUBLE=1), each appending to
# $(CUR_SRCS). Expanded here with the same INC_template, ARCH=aarch64, and
# LONG_DOUBLE_NOT_DOUBLE left unset -- the long-double cut explained above.
SRCS_MK="$BUILD/print_srcs.mk"
SRCS_LIST="$BUILD/srcs.txt"
cat > "$SRCS_MK" <<EOF
OS = NewTOS
ARCH = aarch64
TOP = $TOP
define INC_template
override CUR_SRCS = \$(1)_SRCS
include \$(TOP)/\$(1)/Make.files
SRCS += \$\$(addprefix \$(1)/,\$\$(\$(1)_SRCS))
endef
\$(foreach dir,src aarch64 bsdsrc,\$(eval \$(call INC_template,\$(dir))))
print-srcs:
	@echo \$(SRCS)
EOF
make -s -f "$SRCS_MK" print-srcs | tr ' ' '\n' | grep -v '^$' > "$SRCS_LIST" || {
    echo "error: could not expand OpenLibm's Make.files" >&2; exit 1; }
n_srcs=$(wc -l < "$SRCS_LIST" | tr -d ' ')
[ "$n_srcs" -gt 0 ] || { echo "error: empty source list from Make.files" >&2; exit 1; }

# --- compile -----------------------------------------------------------------
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"
: > "$BUILD/failed-err.txt"   # "<source><TAB><its .err>" for lib_check_failures
ok=0; fail=0
while IFS= read -r c; do
    case "$c" in *.c) ;; *) echo "note: skipping non-C manifest entry $c"; continue;; esac
    d="${c%%/*}"; b="$(basename "${c%.c}")"
    o="$OBJ/${d}__${b}.o"
    if "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$TOP/$c" -o "$o" 2>"$OBJ/${d}__${b}.err"; then
        echo "$c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
        printf '%s\t%s\n' "$c" "$OBJ/${d}__${b}.err" >> "$BUILD/failed-err.txt"
    fi
done < "$SRCS_LIST"

echo "compiled OK=$ok  skipped=$fail  (of $((ok+fail)))"
lib_check_failures "libm" "$BUILD/failed.txt" "$LIBM/expected-failures.txt" \
                   "$BUILD/failed-err.txt" || exit 1

# --- archive -----------------------------------------------------------------
rm -f "$LIBM/libm.a"
"$AR" cr "$LIBM/libm.a" "$OBJ"/*.o || exit 1
echo "archived -> $LIBM/libm.a ($(wc -c < "$LIBM/libm.a" | tr -d ' ') bytes)"

# --- dangling references, measured (docs/sdk/07-mintlib-limits.md section 6) --
# Symbols some member references and no member defines. The libc ones
# (__assert_fail, _ctype, memset) resolve from libc.a; the quad-float
# compiler-rt ones (__addtf3 ...) and the complex-multiply ones
# (__muldc3/__mulsc3) do not resolve anywhere -- a program that pulls one of
# those members fails to link. The list is written to build/dangling.txt and
# documented in docs/sdk/07-mintlib-limits.md.
"$NM" --defined-only "$LIBM/libm.a" 2>/dev/null | awk 'NF>=3 && $1 !~ /:$/ {print $NF}' | sort -u > "$BUILD/defined.txt"
"$NM" -u "$LIBM/libm.a" 2>/dev/null | awk 'NF>=2 && $1 !~ /:$/ && $NF !~ /:$/ {print $NF}' | sort -u > "$BUILD/undefined.txt"
comm -23 "$BUILD/undefined.txt" "$BUILD/defined.txt" > "$BUILD/dangling.txt"
echo "defined symbols: $(wc -l < "$BUILD/defined.txt" | tr -d ' ')  dangling references: $(wc -l < "$BUILD/dangling.txt" | tr -d ' ') (build/dangling.txt)"
echo

# --- CRT + client ------------------------------------------------------------
"$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc \
    -I"$SDK/include" -I"$MINT" -I"$LIBC/mintlib/mintlib" \
    -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1
"$CLANG" -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc \
    -D_GNU_SOURCE \
    -I"$MINT" -I"$LIBM/include" -I"$TOP/include" -isystem "$RES" \
    -c "$CLIENT" -o "$BUILD/client.o" || exit 1

# --- link (crt0 + client + libm.a + libc.a via the NewTOS TPA linker script) --
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why.txt" \
    "$BUILD/crt0.o" "$BUILD/client.o" \
    --start-group "$LIBM/libm.a" "$LIBC/libc.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    "$READOBJ" --file-headers "$OUT" | grep -iE 'Type:|Machine:|Entry:'
    u=$("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)
    echo "undefined symbols in the final image (goal 0): $u"
else
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link.err" | awk '{print $3}' | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/why.txt for the archive pull chains that reference them."
    exit 1
fi
