#!/usr/bin/env bash
#
# build-ldg.sh -- build LDG's client/module library (the "devel" part of LDG,
# Olivier Landemarre, Dominique Bereziat & Arnaud Bercegeay, LGPL 2.1) into a
# NewTOS/AArch64 libldg.a, the way the other ported libraries are built:
# pristine tarball + overlay/ + this script (TASK/LDG-M1,
# docs/analysis/ldg-after-arch03-2026-10-06.md section 4 G2).
#
# SOURCE PIN: sdk/ldg/ldg-svn-r133.tar.xz is the LDG tree that was used, SVN
# trunk r133 (Applications/ldg-src, crawled read-only with curl on
# 2026-10-04, no svn client -- Applications/ldg-src/SOURCE.txt). Its SHA-256
# and the per-file manifest of the tree are in ldg-sources.sha256 /
# ldg-svn-r133.files.sha256. th-otto's ldg-20171014 (with ldg-cross.patch and
# ldg-pwait.patch) was NOT compared; that is an open point of TASK/LDG-M1.
#
# What is compiled: LDG's own object list, src/devel/common.mak
#   OBJS = ldgapp.o ldglib.o ldgutil.o ldgload.o $(EXTRAOBJS)
# WITHOUT gcc.mak's EXTRAOBJS = stdcallgcc.o: stdcall.s is m68k assembly
# (register-preserving trampolines for Pure C / Sozobon callers and
# ldg_callback()). libldg.a for NewTOS therefore has NO ldg_callback symbol
# (lead decision Q5, 2026-10-06: "not present", no stub); a program that
# calls it fails to link instead of misbehaving at run time.
#
# Overlays (sdk/ldg/overlay/, mirroring the LDG tree; each file says why):
#   src/devel/ldgload.c  p_env (32-bit basepage field) -> pointer cast
#   src/devel/ldgutil.c  ldg_cookie via Getcookie(); ldg_cpush empty (the
#                        kernel syncs the caches inside Pexec(3))
# ldgapp.c and ldglib.c compile unmodified.
#
# Flags: the shared SDK freestanding set + LDG's own gcc.mak CFLAGS
# (-fomit-frame-pointer -W -Wall -O2 -I../../include -I.. -I.).
#
# Usage:  sdk/ldg/build-ldg.sh      (LDG_REFRESH=1 forces a re-extract)
# Output: sdk/ldg/libldg.a; headers for programs: sdk/ldg/ldg-r133/include
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LDGDIR="$SDK/ldg"
LIBC="$SDK/libc"
TOP="$LDGDIR/ldg-r133"                                       # extracted tree
TARBALL="$LDGDIR/ldg-svn-r133.tar.xz"
SUMS="$LDGDIR/ldg-sources.sha256"
OVERLAY="$LDGDIR/overlay"
BUILD="$LDGDIR/build"
OBJ="$BUILD/obj"
OUT="$LDGDIR/libldg.a"
mkdir -p "$OBJ"

# --- ensure the ported mintlib libc is present -------------------------------
# libldg.a is compiled against mintlib's headers (osbind.h, mintbind.h,
# mint/basepage.h, mint/cookie.h) and linked with libc.a.
. "$SDK/lib-stale.sh"
lib_ensure libc || exit 1
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ] || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }

# --- ensure the LDG source tree is present (verify + extract + overlay) -------
# Re-extract when forced, when the tree is absent, or when the tarball is
# newer than the tree (a changed pin must not leave the old sources in use --
# the PORT03 lesson recorded in sdk/lib-stale.sh, lib_inputs).
if [ "${LDG_REFRESH:-0}" = "1" ] || [ ! -d "$TOP" ] || [ "$TARBALL" -nt "$TOP" ]; then
  [ -f "$TARBALL" ] || { echo "error: pristine tarball $TARBALL missing" >&2; exit 1; }
  want="$(sed -e 's/#.*//' "$SUMS" | awk -v f="$(basename "$TARBALL")" '$2==f{print $1}')"
  have="$(shasum -a 256 "$TARBALL" | awk '{print $1}')"
  if [ -z "$want" ] || [ "$want" != "$have" ]; then
    echo "error: $(basename "$TARBALL") SHA-256 $have does not match $(basename "$SUMS") ($want)" >&2
    exit 1
  fi
  echo "extracting pristine LDG r133 from $(basename "$TARBALL") (sha256 verified)…"
  rm -rf "$TOP"
  tmp="$(mktemp -d)"
  tar xJf "$TARBALL" -C "$tmp" || { rm -rf "$tmp"; exit 1; }
  mv "$tmp/ldg" "$TOP"
  rmdir "$tmp" 2>/dev/null || true
  touch "$TOP"
fi
if [ -d "$OVERLAY" ]; then
  ( cd "$OVERLAY" && find . -type f ! -name '._*' ! -name '.DS_Store' -print0 | while IFS= read -r -d '' f; do
      mkdir -p "$TOP/$(dirname "$f")"
      cp "$f" "$TOP/$f"
    done )
fi

# --- toolchain ---------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
. "$SDK/toolchain.sh"   # LLVM tools: PATH / LLVM packages first, pinned Rust sysroot as fallback (SDK01 I, K)
AR="${AR:-$(newtos_tool llvm-ar)}"
NM="$(newtos_tool llvm-nm)"
[ -x "$AR" ] || { echo "error: missing LLVM tool: $AR" >&2; exit 1; }

# --- flags -------------------------------------------------------------------
DEVEL="$TOP/src/devel"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fPIE -fno-builtin -nostdinc
        -fomit-frame-pointer -W -Wall)
INCS=(-I"$TOP/include" -I"$TOP/src" -I"$DEVEL" -I"$MINT" -isystem "$RES")

echo "clang : $($CLANG --version | head -1)"
echo "ar    : $AR"
echo

# --- compile: LDG's own OBJS list (src/devel/common.mak), no EXTRAOBJS -------
OBJS_LIST="$(sed -n 's/^OBJS[[:space:]]*=[[:space:]]*//p' "$DEVEL/common.mak" | sed 's/\$(EXTRAOBJS)//')"
[ -n "$OBJS_LIST" ] || { echo "error: could not read OBJS from $DEVEL/common.mak" >&2; exit 1; }
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"
ok=0; fail=0; warn=0
for o in $OBJS_LIST; do
  c="${o%.o}.c"
  if [ -e "$OVERLAY/src/devel/$c" ]; then echo "  overlay: src/devel/$c (from sdk/ldg/overlay/)"; fi
  if "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$DEVEL/$c" -o "$OBJ/$o" 2>"$OBJ/${o%.o}.err"; then
    echo "$c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
    if [ -s "$OBJ/${o%.o}.err" ]; then
      warn=$((warn+1)); echo "  warnings in $c:"; sed 's/^/      /' "$OBJ/${o%.o}.err"
    fi
  else
    echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$OBJ/$o"
  fi
done
echo "compiled OK=$ok  FAILED=$fail  (files with warnings: $warn)"
if [ "$fail" -ne 0 ]; then
  echo
  echo "STOP: LDG sources that did not compile (fix the toolchain or the overlay):"
  while IFS= read -r f; do
    echo "  === $f ==="
    sed 's/^/      /' "$OBJ/${f%.c}.err" | head -20
  done < "$BUILD/failed.txt"
  exit 1
fi

# --- archive -----------------------------------------------------------------
rm -f "$OUT"
"$AR" cr "$OUT" "$OBJ"/*.o || exit 1
echo "archived -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
"$NM" --defined-only -g "$OUT" 2>/dev/null | awk 'NF>=3 {print $NF}' | sort -u > "$BUILD/defined.txt"
echo "exported symbols: $(tr '\n' ' ' < "$BUILD/defined.txt")"
echo
