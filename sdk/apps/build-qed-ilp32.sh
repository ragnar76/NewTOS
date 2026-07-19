#!/usr/bin/env bash
#
# build-qed-ilp32.sh -- compile QED (a pure-C GEM text editor, tho-otto layout)
# into a native NewTOS/AArch64 **ELF32** GEM application (QED.NAP) using the GNU
# ILP32 toolchain, against the ILP32-ported mintlib (libc-ilp32.a) + gemlib
# (gemlib-ilp32.a) + cflib (cflib-ilp32.a). This is the GNU-ILP32 sibling of
# build-qed.sh and the payoff of the LP64->ILP32 app-ABI pivot (ADR-020, stage 4):
#
#   QED's own C code assumes sizeof(pointer) == 4 (its memory.c bin-allocator, its
#   OBJECT/OBSPEC use). That is TRUE on ILP32, so QED's 37 src/*.c compile+link
#   100% UNMODIFIED. Every compile error or undefined symbol is a TOOLCHAIN gap,
#   fixed in the ILP32 libraries / overlays -- NEVER in QED. If a QED .c/.h needs
#   editing, STOP and report it: a needed QED edit means the ILP32 approach failed
#   for that spot, which is a lead decision.
#
#     tree/objs : sdk/apps/qed/build-ilp32/   (vs LP64 sdk/apps/qed/build/)
#     output    : sdk/apps/qed/build-ilp32/QED.NAP  (ELF32 AArch64)
#
# Toolchain (GNU bare-metal AArch64, Homebrew aarch64-elf-*), identical to
# build-{libc,gemlib,cflib}-ilp32.sh:
#   - aarch64-elf-gcc -mabi=ilp32     -> 4-byte long/pointer, ELF32 objects.
#     (GCC 16 prints "-mabi=ilp32 is deprecated"; it is the only spelling.)
#   - aarch64-elf-ar                  -> archive.
#   - aarch64-elf-ld -m aarch64elf32  -> ELF32 AArch64 static executable.
#
# QED's own build (sdk/apps/qed/{Makefile,CONFIGVARS,RULES,src/Makefile.objs})
# uses, for a cross build:
#   CFLAGS += -funsigned-char $(INCLUDES) $(M68K_ATARI_MINT_CFLAGS)
#           += -O2 -finline-functions -fomit-frame-pointer   (OPTS)
#   LIBS    = -lcflib -lgem -liio                             (Makefile.objs)
# We replace the m68k flag bundle with the AArch64/ILP32 freestanding set (shared
# with the other build-*-ilp32.sh), keep -funsigned-char + -O2, and add
# -D__NEWTOS__ so the gemlib/cflib overlay headers select their retargeted (svc,
# direct-call) paths. -liio (integer-only stdio) is subsumed by libc-ilp32.a. The
# source manifest is QED's own COBJS (src/SRCFILES).
#
# Usage:  sdk/apps/build-qed-ilp32.sh
#
# Toolchain override:
#   GCC=/path/to/aarch64-elf-gcc LD=... AR=... sdk/apps/build-qed-ilp32.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
QED="$SDK/apps/qed"
QSRC="$QED/src"
CF="$SDK/cflib"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
CFTOP="$CF/cflib-21-ilp32"
GEMTOP="$GEM/gemlib-0.44.0-ilp32"
MINT="$LIBC/mintlib-ilp32/include"
BUILD="$QED/build-ilp32"
OBJ="$BUILD/obj"
OUT="$BUILD/QED.NAP"
mkdir -p "$OBJ"

# --- locate the GNU bare-metal AArch64 toolchain -----------------------------
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
NM="$(find_tool "${NM:-}" aarch64-elf-nm)" || NM=""

# --- ensure the three ILP32 libraries are present ----------------------------
if [ ! -f "$LIBC/libc-ilp32.a" ]; then
  echo "libc-ilp32.a absent -- running build-libc-ilp32.sh first…"
  "$LIBC/build-libc-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-libc-ilp32.sh failed" >&2; exit 1; }
fi
if [ ! -f "$GEM/gemlib-ilp32.a" ]; then
  echo "gemlib-ilp32.a absent -- running build-gemlib-ilp32.sh first…"
  "$GEM/build-gemlib-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-gemlib-ilp32.sh failed" >&2; exit 1; }
fi
if [ ! -f "$CF/cflib-ilp32.a" ]; then
  echo "cflib-ilp32.a absent -- running build-cflib-ilp32.sh first…"
  "$CF/build-cflib-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-cflib-ilp32.sh failed" >&2; exit 1; }
fi
[ -d "$MINT" ]   || { echo "error: ILP32 mintlib headers missing ($MINT); run build-libc-ilp32.sh" >&2; exit 1; }
[ -d "$GEMTOP" ] || { echo "error: ILP32 gemlib headers missing ($GEMTOP); run build-gemlib-ilp32.sh" >&2; exit 1; }
[ -d "$CFTOP" ]  || { echo "error: ILP32 cflib headers missing ($CFTOP); run build-cflib-ilp32.sh" >&2; exit 1; }
[ -d "$QSRC" ]   || { echo "error: QED source missing ($QSRC)" >&2; exit 1; }

# --- flags (QED CONFIGVARS/OPTS, retargeted for GNU ILP32) -------------------
# QED's -funsigned-char + -O2, plus the AArch64/ILP32 freestanding/no-libc set
# and -D__NEWTOS__ to reach the retargeted GEM headers. -Wno-builtin-macro-
# redefined silences cflib/QED's intentional TRUE/FALSE re-defines under gcc.
GINC="$("$GCC" -mabi=ilp32 -print-file-name=include)"        # gcc fixed includes
CFLAGS=(-mabi=ilp32 -ffreestanding -nostdlib -O2
        -fno-pic -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__
        -Wno-builtin-macro-redefined)
# Include order: QED src, then cflib top (cflib.h), gemlib top (mt_gem.h),
# ILP32 mintlib include (stdio.h/mintbind.h), gcc fixed includes last.
INCS=(-I"$QSRC" -I"$CFTOP" -I"$GEMTOP" -I"$MINT" -isystem "$GINC")

echo "gcc : $("$GCC" -mabi=ilp32 --version 2>/dev/null | head -1)"
echo "ar  : $AR"
echo "ld  : $("$LD" --version | head -1)"
echo "abi : ILP32 (4-byte long/pointer, ELF32 AArch64)"
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
    if "$GCC" "${CFLAGS[@]}" "${INCS[@]}" -c "$src" -o "$o" 2>"$OBJ/${b}.err"; then
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
    echo "STOP: fix the toolchain (ILP32 libc/gemlib/cflib, newtos-arch shim, or overlay header) and re-run."
    exit 1
fi

# --- CRT ---------------------------------------------------------------------
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" \
    -c "$LIBC/crt0-ilp32.S" -o "$BUILD/crt0.o" || exit 1

# --- link (crt0 + all QED objects + cflib + gemlib + libc) -------------------
# cflib -> gemlib -> libc dependency order; wrap in a group so any residual
# back-reference between the three archives still resolves. `-m aarch64elf32`
# selects the ELF32 AArch64 output format from newtos.ld.
rm -f "$OUT"
"$LD" -m aarch64elf32 -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --no-warn-rwx-segments -Map "$BUILD/link.map" \
    "$BUILD/crt0.o" "$OBJ"/*.o \
    --start-group "$CF/cflib-ilp32.a" "$GEM/gemlib-ilp32.a" "$LIBC/libc-ilp32.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    [ -n "$READELF" ] && "$READELF" -h "$OUT" | grep -iE 'Class:|Data:|Type:|Machine:|Entry'
    echo
    if [ -n "$NM" ]; then
        u=$("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)
        echo "undefined symbols in the final image (goal 0): $u"
    fi
else
    echo
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined reference to \`[^']+'" "$BUILD/link.err" \
        | sed "s/.*\`//; s/'//" | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/link.err and $BUILD/link.map for detail."
    exit 1
fi
