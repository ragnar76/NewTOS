#!/usr/bin/env bash
#
# build_before.sh <outdir>  -- build the two "before" programs for PORT01.
#
# "Before" means: the committed crt0 (HEAD 45de38b, `git show` -- read-only,
# the shared tree is never touched by git), which sets argc = 1 and
# argv = { "", NULL } unconditionally. Everything else is identical:
#
#   ARGVOLD.NAP  = crt0-before.o + the SAME argvtest.o + the SAME libc.a
#   QEDOLD.NAP   = crt0-before.o + QED's ALREADY COMPILED objects
#                  (sdk/apps/qed/build/obj/*.o) + the same four archives
#
# so the only variable between the before and after pictures is the C runtime
# startup. QED itself is not recompiled and not modified (project rule: fixes
# go in sdk/, never in the application).
#
# The flags are copied from sdk/libc/build-libc.sh (crt0 + a libc client) and
# sdk/apps/build-qed.sh (the link line).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
OUT="${1:?usage: build_before.sh <outdir>}"
mkdir -p "$OUT"
W="$OUT/before-obj"; mkdir -p "$W"

SDK="$WS/sdk"
LIBC="$SDK/libc"
MINTTOP="$LIBC/mintlib"
MINT="$MINTTOP/include"
GEMTOP="$SDK/gemlib/gemlib-0.44.0"
CFTOP="$SDK/cflib/cflib-21"
LIBMTOP="$SDK/libm/openlibm-0.8.8"

CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
LLD="$BIN/rust-lld"
NM="$BIN/llvm-nm"

# --- the committed crt0 (read-only git access) -------------------------------
( cd "$WS" && git show HEAD:sdk/libc/crt0.S ) > "$W/crt0-before.S"
grep -q '__crt0_argv' "$W/crt0-before.S" || {
  echo "error: HEAD's crt0.S does not carry the hardcoded argv" >&2; exit 1; }

BASE=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
      -fno-pic -fno-builtin -nostdinc)
LIBCDEFS=(-D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE
          -D__NO_LONG_DOUBLE_MATH -Wno-macro-redefined)
LIBCINCS=(-I"$MINTTOP" -I"$MINT" -I"$MINTTOP/mintlib" -I"$MINTTOP/stdlib"
          -isystem "$RES")

"$CLANG" "${BASE[@]}" "${LIBCDEFS[@]}" -I"$SDK/include" -I"$MINT" -I"$MINTTOP/mintlib" \
    -c "$W/crt0-before.S" -o "$W/crt0-before.o"

# --- ARGVOLD.NAP -------------------------------------------------------------
"$CLANG" "${BASE[@]}" "${LIBCDEFS[@]}" -I"$SDK" "${LIBCINCS[@]}" \
    -c "$SDK/examples/argvtest.c" -o "$W/argvtest.o"
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none "$W/crt0-before.o" "$W/argvtest.o" "$LIBC/libc.a" \
    -o "$OUT/ARGVOLD.NAP"
echo "ARGVOLD.NAP  $(wc -c < "$OUT/ARGVOLD.NAP" | tr -d ' ') bytes, undefined: $("$NM" -u "$OUT/ARGVOLD.NAP" | grep -c . || true)"

# --- QEDOLD.NAP: QED's own objects, only the crt0 swapped --------------------
QOBJ="$SDK/apps/qed/build/obj"
[ -d "$QOBJ" ] || { echo "error: QED not built ($QOBJ); run sdk/apps/build-qed.sh" >&2; exit 1; }
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none "$W/crt0-before.o" "$QOBJ"/*.o \
    --start-group "$SDK/cflib/cflib.a" "$SDK/gemlib/gemlib.a" \
                  "$SDK/libm/libm.a" "$LIBC/libc.a" --end-group \
    -o "$OUT/QEDOLD.NAP"
echo "QEDOLD.NAP   $(wc -c < "$OUT/QEDOLD.NAP" | tr -d ' ') bytes, undefined: $("$NM" -u "$OUT/QEDOLD.NAP" | grep -c . || true)"

# Sanity: the before images must NOT contain the new entry point, the after
# ones must.
for f in "$OUT/ARGVOLD.NAP" "$OUT/QEDOLD.NAP"; do
  if "$NM" "$f" | grep -q '__libc_parseargs'; then
    echo "error: $f pulled __libc_parseargs -- not a 'before' image" >&2; exit 1
  fi
done
echo "before images carry no __libc_parseargs (as intended)"
