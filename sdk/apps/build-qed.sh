#!/usr/bin/env bash
#
# build-qed.sh -- compile QED (a pure-C GEM text editor, tho-otto layout) into a
# native NewTOS/AArch64 GEM application (.NAP, per ADR-018) against the ported
# mintlib (libc.a) + gemlib (gemlib.a) + cflib (cflib.a).
#
# QED is the acceptance test for ADR-019 ("port real Atari C libraries, don't
# reimplement"): its 37 src/*.c compile 100% UNMODIFIED. Every compile error or
# undefined symbol is a TOOLCHAIN gap, fixed in the libraries -- NEVER in QED.
# QED has ZERO assembly, so the one allowed app edit (m68k asm retarget) does not
# apply here: nothing under sdk/apps/qed/ is touched by this script.
#
# QED's own build (sdk/apps/qed/{Makefile,CONFIGVARS,RULES,src/Makefile.objs})
# uses, for a cross build:
#   CFLAGS += -funsigned-char $(INCLUDES) $(M68K_ATARI_MINT_CFLAGS)
#           += -O2 -finline-functions -fomit-frame-pointer   (OPTS)
#   LIBS    = -lcflib -lgem -liio                             (Makefile.objs)
# We replace the m68k flag bundle with our AArch64 freestanding set (matching
# the other build-*.sh), keep -funsigned-char + -O2, and add -D__NEWTOS__ so the
# gemlib/cflib overlay headers select their retargeted (svc, direct-call) paths.
# -liio (integer-only stdio) is subsumed by the full printf in libc.a, so we link
# libc.a directly. The source manifest is QED's own COBJS (src/SRCFILES).
#
# Usage:  sdk/apps/build-qed.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
QED="$SDK/apps/qed"
QSRC="$QED/src"
CF="$SDK/cflib"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
CFTOP="$CF/cflib-21"
GEMTOP="$GEM/gemlib-0.44.0"
BUILD="$QED/build"
OBJ="$BUILD/obj"
OUT="$BUILD/QED.NAP"
mkdir -p "$OBJ"

# --- ensure the three ported libraries are present --------------------------
if [ ! -f "$LIBC/libc.a" ]; then
  echo "libc.a absent -- running build-libc.sh first…"
  "$LIBC/build-libc.sh" >/dev/null 2>&1 || { echo "error: build-libc.sh failed" >&2; exit 1; }
fi
if [ ! -f "$GEM/gemlib.a" ]; then
  echo "gemlib.a absent -- running build-gemlib.sh first…"
  "$GEM/build-gemlib.sh" >/dev/null 2>&1 || { echo "error: build-gemlib.sh failed" >&2; exit 1; }
fi
if [ ! -f "$CF/cflib.a" ]; then
  echo "cflib.a absent -- running build-cflib.sh first…"
  "$CF/build-cflib.sh" >/dev/null 2>&1 || { echo "error: build-cflib.sh failed" >&2; exit 1; }
fi
MINT="$LIBC/mintlib/include"
[ -d "$MINT" ]   || { echo "error: mintlib headers missing ($MINT); run build-libc.sh" >&2; exit 1; }
[ -d "$GEMTOP" ] || { echo "error: gemlib headers missing ($GEMTOP); run build-gemlib.sh" >&2; exit 1; }
[ -d "$CFTOP" ]  || { echo "error: cflib headers missing ($CFTOP); run build-cflib.sh" >&2; exit 1; }
[ -d "$QSRC" ]   || { echo "error: QED source missing ($QSRC)" >&2; exit 1; }

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

# --- flags -------------------------------------------------------------------
# Freestanding/no-libc set (shared with the other build-*.sh) + QED's own
# -funsigned-char + -O2, and -D__NEWTOS__ to reach the retargeted GEM headers.
# -Wno-macro-redefined silences cflib/QED's intentional TRUE/FALSE re-defines.
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fno-pic -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__
        -Wno-macro-redefined)
# Include order: QED src, then cflib top (cflib.h), gemlib top (gem.h/macros.h),
# mintlib include (stdio.h/mintbind.h), clang freestanding headers last.
INCS=(-I"$QSRC" -I"$CFTOP" -I"$GEMTOP" -I"$MINT" -isystem "$RES")

echo "clang : $($CLANG --version | head -1)"
echo "ar    : $AR"
echo "lld   : $LLD"
echo

# --- authoritative file list: QED's own COBJS manifest (src/SRCFILES) --------
COBJS_MK="$BUILD/print_cobjs.mk"
COBJS_LIST="$BUILD/cobjs.txt"
{ echo 'empty ='; echo 'include SRCFILES'; echo 'print:'; printf '\t@printf "%%s\\n" $(COBJS)\n'; } > "$COBJS_MK"
( cd "$QSRC" && make -f "$COBJS_MK" print 2>/dev/null ) | tr ' ' '\n' | grep -E '\.c$' | sed 's,^\./,,' | sort -u > "$COBJS_LIST"
n_cobjs=$(wc -l < "$COBJS_LIST" | tr -d ' ')
[ "$n_cobjs" -gt 0 ] || { echo "error: could not expand COBJS from QED src/SRCFILES" >&2; exit 1; }
echo "QED COBJS manifest: $n_cobjs files"

# --- compile all QED sources (must be 100% -- QED is unmodified) -------------
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"
ok=0; fail=0
while IFS= read -r c; do
    [ -n "$c" ] || continue
    src="$QSRC/$c"
    [ -e "$src" ] || { echo "$c (missing)" >> "$BUILD/failed.txt"; fail=$((fail+1)); continue; }
    b="${c%.c}"
    o="$OBJ/${b}.o"
    if "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" -c "$src" -o "$o" 2>"$OBJ/${b}.err"; then
        echo "$c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
    fi
done < "$COBJS_LIST"

echo "compiled OK=$ok  FAILED=$fail  (of $n_cobjs)"
if [ "$fail" -ne 0 ]; then
    echo
    echo "QED files that FAILED to compile (each is a TOOLCHAIN gap to fix, NOT a QED edit):"
    while IFS= read -r f; do
        echo "  === $f ==="
        sed 's/^/      /' "$OBJ/${f%.c}.err" 2>/dev/null | head -20
    done < "$BUILD/failed.txt"
    echo
    echo "STOP: fix the toolchain (libc/gemlib/cflib objects, newtos-arch shim, or overlay header) and re-run."
    exit 1
fi

# --- CRT ---------------------------------------------------------------------
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1

# --- link (crt0 + all QED objects + cflib.a + gemlib.a + libc.a) -------------
# cflib -> gemlib -> libc dependency order; wrap in a group so any residual
# back-reference between the three archives still resolves.
rm -f "$OUT"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why.txt" \
    "$BUILD/crt0.o" "$OBJ"/*.o \
    --start-group "$CF/cflib.a" "$GEM/gemlib.a" "$LIBC/libc.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    "$READOBJ" --file-headers "$OUT" | grep -iE 'Class:|DataEncoding:|Type:|Machine:|Entry:|Version'
    echo
    # Count real undefined references. (llvm-nm -u excludes the mandatory ELF
    # null symbol at index 0, which readobj would otherwise report as
    # "Undefined".)
    NM="$BIN/llvm-nm"
    u=$("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)
    echo "undefined symbols in the final image (goal 0): $u"
else
    echo
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link.err" | awk '{print $3}' | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/why.txt for the archive pull chains that reference them."
    exit 1
fi
