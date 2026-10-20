#!/usr/bin/env bash
# x18-experiment.sh -- why sdk/compiler-rt/tests/rt-host.sh runs a -ffixed-x18 rebuild: the same test program, once
# linked against the SHIPPED libm.a + libclang_rt.builtins.a (both use x18 as an ordinary register) and once against
# the -ffixed-x18 rebuild of the same sources, each run RUNS (default 10) times by the same rt-host.c on the macOS host (which does not
# preserve x18 for user code). Output -> ../checks/x18-experiment.txt
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SDK="$WS/sdk"; W="$S/x18exp"; rm -rf "$W"; mkdir -p "$W/x18/crt" "$W/x18/libm" "$W/olh"
. "$SDK/toolchain.sh"; BIN="$(newtos_rust_bindir)"; RES="$(clang -print-resource-dir)/include"
CRT_TOP="$SDK/compiler-rt/compiler-rt-21.1.8.src"; OL_TOP="$SDK/libm/openlibm-0.8.8"; MINT="$SDK/libc/mintlib/include"
while IFS= read -r c; do o="$(printf '%s' "${c%.c}" | tr '/' '_').o"
  clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc -std=c11 -fvisibility=hidden \
    -DVISIBILITY_HIDDEN -fomit-frame-pointer -DCOMPILER_RT_HAS_FLOAT16 -ffixed-x18 -I"$CRT_TOP/lib/builtins" -isystem "$RES" \
    -c "$CRT_TOP/$c" -o "$W/x18/crt/$o" || exit 1
done < "$SDK/compiler-rt/build/srcs.txt"
while IFS= read -r c; do d="${c%%/*}"; o="${d}__$(basename "${c%.c}").o"
  clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc -std=c99 -fno-strict-aliasing \
    -D__BSD_VISIBLE -ffixed-x18 -I"$OL_TOP" -I"$OL_TOP/include" -I"$OL_TOP/aarch64" -I"$OL_TOP/src" -I"$MINT" -isystem "$RES" \
    -c "$OL_TOP/$c" -o "$W/x18/libm/$o" 2>/dev/null || exit 1
  cc -O2 -std=c99 -fno-builtin -fno-strict-aliasing -D__BSD_VISIBLE -Wno-implicit-function-declaration \
    -I"$OL_TOP" -I"$OL_TOP/include" -I"$OL_TOP/aarch64" -I"$OL_TOP/src" -c "$OL_TOP/$c" -o "$W/olh/$o" 2>/dev/null || exit 1
done < "$SDK/libm/build/srcs.txt"
"$BIN/llvm-ar" cr "$W/x18-builtins.a" "$W"/x18/crt/*.o; "$BIN/llvm-ar" cr "$W/x18-libm.a" "$W"/x18/libm/*.o; ar cr "$W/olh.a" "$W"/olh/*.o
sed 's/^\(    \)\.init_array : {/\1. = ALIGN(0x4000);\
\1.init_array : {/' "$SDK/newtos.ld" > "$W/host.ld"
"$SDK/bin/newtos-cc" -c "$SDK/libc/crt0.S" -o "$W/crt0.o"; "$SDK/bin/newtos-cc" -lm -c "$SDK/compiler-rt/tests/rt-elf.c" -o "$W/rt-elf.o"
lk() { "$BIN/rust-lld" -flavor gnu -T "$W/host.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
  "$W/crt0.o" "$W/rt-elf.o" --start-group "$1" "$SDK/libc/libc.a" "$2" --end-group -o "$3"; }
lk "$SDK/libm/libm.a" "$SDK/compiler-rt/libclang_rt.builtins.a" "$W/RT-shipped.elf" || exit 1
lk "$W/x18-libm.a" "$W/x18-builtins.a" "$W/RT-x18.elf" || exit 1
cc -O1 -o "$W/rt-host" "$SDK/compiler-rt/tests/rt-host.c" "$W/olh.a" || exit 1
OUT="$HERE/../checks/x18-experiment.txt"; : > "$OUT"
for v in shipped x18; do
  echo "x18 instructions in the linked $v program: $("$BIN/llvm-objdump" -d "$W/RT-$v.elf" | grep -E '^ +[0-9a-f]+:' | grep -cE '\b[xw]18\b')" | tee -a "$OUT"
  for i in $(seq 1 ${RUNS:-10}); do
    echo "== $v run $i" | tee -a "$OUT"
    "$W/rt-host" "$W/RT-$v.elf" 2>&1 | grep -E 'binary128 /|FAIL tf|mismatches$|outside|rt-host:' | grep -vE 'binary128 [+*-]:' | tee -a "$OUT"
  done
done
