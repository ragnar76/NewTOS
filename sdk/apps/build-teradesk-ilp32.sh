#!/usr/bin/env bash
#
# build-teradesk-ilp32.sh -- compile TeraDesk (tho-otto/aRTS TeraDesk 4.08, a
# pure-C GEM desktop replacement) into a native NewTOS/AArch64 **ELF32** GEM
# application (DESKTOP.NAP) using the GNU ILP32 toolchain, against the
# ILP32-ported mintlib (libc-ilp32.a) + gemlib (gemlib-ilp32.a). TeraDesk bundles
# its own dialog/utility libraries (library/xdialog + library/utility), so it does
# NOT need cflib -- unlike QED. This is a second, independent EL0 GEM-app validator
# of the app-facing AES/VDI/GEMDOS svc + ILP32 path (see the desktop-bypasses-app-
# abi-path finding): if TeraDesk renders correctly, AES/VDI is sound; if it breaks
# like QED, the bug is in the kernel svc/ILP32 path, not in any one app.
#
#   TeraDesk's own build (Makefile + support/config.am) uses, for a cross build:
#     OPTS   = -Os -fomit-frame-pointer
#     CFLAGS = $(OPTS) -I library/xdialog -I library/utility <warnings> -m68000
#     LIBS   = -lgem
#   We replace the m68k flag with the AArch64/ILP32 freestanding set (shared with
#   build-qed-ilp32.sh), drop -Werror (retargeting/ABI warnings must not fail the
#   build), and add -D__NEWTOS__ so the gemlib overlay headers select their
#   retargeted (svc, direct-call) paths.
#
#   TeraDesk is UNMODIFIED except its m68k inline asm, which must be retargeted to
#   AArch64 or stubbed honestly (NatFeats do not exist here) -- the same rule as
#   QED. Inline asm lives in: library/utility/nf_ops.c (NatFeats), library/utility/
#   cookie.c (cookie jar), resource.c, file.c. If a NON-asm .c fails, it is a
#   TOOLCHAIN gap (gemlib/libc/overlay), NOT a TeraDesk edit -- STOP and report it.
#
#     tree/objs : sdk/apps/teradesk/build-ilp32/
#     output    : sdk/apps/teradesk/build-ilp32/DESKTOP.NAP  (ELF32 AArch64)
#
# Usage:  sdk/apps/build-teradesk-ilp32.sh
#   Toolchain override: GCC=/path/to/aarch64-elf-gcc LD=... AR=... build-teradesk-ilp32.sh
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
TERA="$SDK/apps/teradesk"
GEM="$SDK/gemlib"
LIBC="$SDK/libc"
GEMTOP="$GEM/gemlib-0.44.0-ilp32"
MINT="$LIBC/mintlib-ilp32/include"
BUILD="$TERA/build-ilp32"
OBJ="$BUILD/obj"
OUT="$BUILD/DESKTOP.NAP"
mkdir -p "$OBJ"

# --- locate the GNU bare-metal AArch64 toolchain -----------------------------
find_tool() { local override="$1" name="$2"
    if [ -n "$override" ]; then echo "$override"; return 0; fi
    if command -v "$name" >/dev/null 2>&1; then echo "$name"; return 0; fi
    if [ -x "/opt/homebrew/bin/$name" ]; then echo "/opt/homebrew/bin/$name"; return 0; fi
    return 1
}
GCC="$(find_tool "${GCC:-}" aarch64-elf-gcc)" || { echo "error: aarch64-elf-gcc not found (brew install aarch64-elf-gcc)" >&2; exit 1; }
LD="$(find_tool "${LD:-}"  aarch64-elf-ld)"  || { echo "error: aarch64-elf-ld not found" >&2; exit 1; }
READELF="$(find_tool "${READELF:-}" aarch64-elf-readelf)" || READELF=""
NM="$(find_tool "${NM:-}" aarch64-elf-nm)" || NM=""

# --- ensure the two ILP32 libraries are present ------------------------------
if [ ! -f "$LIBC/libc-ilp32.a" ]; then
  echo "libc-ilp32.a absent -- running build-libc-ilp32.sh first…"
  "$LIBC/build-libc-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-libc-ilp32.sh failed" >&2; exit 1; }
fi
if [ ! -f "$GEM/gemlib-ilp32.a" ]; then
  echo "gemlib-ilp32.a absent -- running build-gemlib-ilp32.sh first…"
  "$GEM/build-gemlib-ilp32.sh" >/dev/null 2>&1 || { echo "error: build-gemlib-ilp32.sh failed" >&2; exit 1; }
fi
[ -d "$MINT" ]   || { echo "error: ILP32 mintlib headers missing ($MINT)" >&2; exit 1; }
[ -d "$GEMTOP" ] || { echo "error: ILP32 gemlib headers missing ($GEMTOP)" >&2; exit 1; }
[ -d "$TERA" ]   || { echo "error: TeraDesk source missing ($TERA)" >&2; exit 1; }

# --- flags (TeraDesk OPTS, retargeted for GNU ILP32) -------------------------
GINC="$("$GCC" -mabi=ilp32 -print-file-name=include)"
CFLAGS=(-mabi=ilp32 -ffreestanding -nostdlib -Os -fomit-frame-pointer
        -fno-pic -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__
        -Wno-builtin-macro-redefined)
# Include order: TeraDesk root, its xdialog + utility libs, gemlib top (gem.h/
# portaes.h/portvdi.h/tos.h), ILP32 mintlib include, gcc fixed includes last.
INCS=(-I"$TERA" -I"$TERA/library/xdialog" -I"$TERA/library/utility"
      -I"$GEMTOP" -I"$MINT" -isystem "$GINC")

echo "gcc : $("$GCC" -mabi=ilp32 --version 2>/dev/null | head -1)"
echo "abi : ILP32 (4-byte long/pointer, ELF32 AArch64)"
echo "libs: gemlib-ilp32.a + libc-ilp32.a  (no cflib -- TeraDesk bundles xdialog)"
echo

# --- file list: every TeraDesk .c except library/utility/malloc.c (not in SRCS,
#     libc provides malloc). Faithful to the Makefile SRCS manifest (48 files). --
mapfile -t CFILES < <(cd "$TERA" && find . -name '*.c' ! -path './library/utility/malloc.c' | sed 's,^\./,,' | sort)
n=${#CFILES[@]}
echo "TeraDesk sources: $n .c files"

# --- compile all -------------------------------------------------------------
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"
ok=0; fail=0
for c in "${CFILES[@]}"; do
    src="$TERA/$c"
    o="$OBJ/$(echo "$c" | tr '/' '_' | sed 's,\.c$,.o,')"
    e="${o%.o}.err"
    if "$GCC" "${CFLAGS[@]}" "${INCS[@]}" -c "$src" -o "$o" 2>"$e"; then
        echo "$c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
    else
        echo "$c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
    fi
done
echo "compiled OK=$ok  FAILED=$fail  (of $n)"

if [ "$fail" -ne 0 ]; then
    echo
    echo "FAILED files (asm -> retarget; non-asm -> TOOLCHAIN gap, not a TeraDesk edit):"
    while IFS= read -r f; do
        o="$OBJ/$(echo "$f" | tr '/' '_' | sed 's,\.c$,.o,')"
        echo "  === $f ==="
        head -8 "${o%.o}.err" 2>/dev/null | sed 's/^/      /'
    done < "$BUILD/failed.txt"
    echo
    echo "STOP: first compile pass. Triage the failures before linking."
    exit 1
fi

# --- CRT + link (only reached once every .c compiles) ------------------------
"$GCC" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0-ilp32.S" -o "$BUILD/crt0.o" || exit 1
rm -f "$OUT"
"$LD" -m aarch64elf32 -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --no-warn-rwx-segments -Map "$BUILD/link.map" \
    "$BUILD/crt0.o" "$OBJ"/*.o \
    --start-group "$GEM/gemlib-ilp32.a" "$LIBC/libc-ilp32.a" --end-group \
    -o "$OUT" 2>"$BUILD/link.err"
if [ $? -eq 0 ]; then
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    [ -n "$READELF" ] && "$READELF" -h "$OUT" | grep -iE 'Class:|Data:|Type:|Machine:|Entry'
    [ -n "$NM" ] && echo "undefined symbols (goal 0): $("$NM" -u "$OUT" 2>/dev/null | grep -c . || true)"
else
    echo "LINK FAILED -- residual undefined symbols:"
    grep -oE "undefined reference to \`[^']+'" "$BUILD/link.err" | sed "s/.*\`//; s/'//" | sort -u | sed 's/^/    /'
    exit 1
fi
