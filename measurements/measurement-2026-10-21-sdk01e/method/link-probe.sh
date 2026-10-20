#!/bin/bash
# probe.sh OUTDIR SYMFILE|- LIB...  -- per-symbol link probe (method of
# docs/analysis/sdk-inventory-2026-10-20.md section 0 step 2).
# SYMFILE "-" = derive public symbols from the first LIB.
set -u
OUT=$1; shift; SYMS=$1; shift
ROOT=/Volumes/Daten/Development/PiRusTOS
SYSROOT="$(rustc --print sysroot)"; BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
NM=$BIN/llvm-nm; LLD=$BIN/rust-lld
mkdir -p $OUT/lk
if [ "$SYMS" = "-" ]; then
  $NM -g --defined-only "$1" 2>/dev/null | awk 'NF==3 && $2 ~ /^[TDBRCVWS]$/ {print $3}' | sort -u > $OUT/pub.txt
  SYMS=$OUT/pub.txt
fi
CRT=$OUT/lk/crt0.o
clang -target aarch64-unknown-none-elf -ffreestanding -fPIE -I$ROOT/sdk/include -I$ROOT/sdk/libc/mintlib/include -I$ROOT/sdk/libc/mintlib/mintlib -c $ROOT/sdk/libc/crt0.S -o $CRT || exit 1
: > $OUT/linkfail.txt
while read sym; do
  printf 'extern char %s; void *volatile __ref = (void*)&%s; int main(void){return 0;}\n' "$sym" "$sym" > $OUT/lk/r.c
  clang -target aarch64-unknown-none-elf -ffreestanding -fPIE -c $OUT/lk/r.c -o $OUT/lk/r.o 2>/dev/null || { echo "CCFAIL $sym" >> $OUT/linkfail.txt; continue; }
  if ! $LLD -flavor gnu -T $ROOT/sdk/newtos.ld -pie --no-dynamic-linker -z max-page-size=0x1000 --build-id=none $CRT $OUT/lk/r.o --start-group "$@" --end-group -o $OUT/lk/r.elf 2>$OUT/lk/err.txt; then
    echo "FAIL $sym: $(grep -oE 'undefined symbol: [^ ]+|duplicate symbol: [^ ]+' $OUT/lk/err.txt | sort -u | tr '\n' ' ')" >> $OUT/linkfail.txt
  fi
done < $SYMS
echo "symbols=$(wc -l < $SYMS) fail=$(grep -c '' $OUT/linkfail.txt)"
