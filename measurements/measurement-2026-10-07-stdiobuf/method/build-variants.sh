#!/usr/bin/env bash
# build-variants.sh <scratch> -- exploration (TASK/FS08 part 4): FSBENCH
# builds whose libc differs ONLY in st_blksize for a regular file.
#   libc-1k.a  = libc.a as it was before this change (copied before the edit)
#   libc-NNk.a = that archive with arch__do_fstat.o replaced by the CHANGED
#                newtos-arch/do_fstat.c, NEWTOS_FILE_BLKSIZE sed'ed to NN KiB
# and two FSBENCH flavours per libc: full (sdk/examples/fsbench.c as tracked)
# and L3-only (scratch copy with run_l1/run_l2 removed from main()).
set -euo pipefail
# $1 = scratch root; $1/before/libc.a = the archive saved before the edit
S="$1"; WS=/Volumes/Daten/Development/PiRusTOS; SDK=$WS/sdk; TOP=$SDK/libc/mintlib
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
RES="$(clang -print-resource-dir)/include"
O="$S/explore/o"; mkdir -p "$O"
CF=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc
    -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH -Wno-macro-redefined)
INCS=(-I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")
cp "$S/before/libc.a" "$O/libc-1k.a"
for k in 16 32 64; do
  sed "s/^#define NEWTOS_FILE_BLKSIZE.*/#define NEWTOS_FILE_BLKSIZE	(${k}UL * 1024UL)/" \
      "$SDK/libc/newtos-arch/do_fstat.c" > "$O/do_fstat-$k.c"
  grep -n "define NEWTOS_FILE_BLKSIZE" "$O/do_fstat-$k.c"
  clang "${CF[@]}" -I"$SDK/include" "${INCS[@]}" -I"$SDK/libc/newtos-arch" -c "$O/do_fstat-$k.c" -o "$O/arch__do_fstat.o"
  cp "$S/before/libc.a" "$O/libc-${k}k.a"
  "$BIN/llvm-ar" r "$O/libc-${k}k.a" "$O/arch__do_fstat.o"
done
sed -e 's/^\t\trun_l1(p);$/\t\t\/* L3-only scratch variant: run_l1(p); *\//' \
    -e 's/^\t\trun_l2(p);$/\t\t\/* L3-only scratch variant: run_l2(p); *\//' \
    "$SDK/examples/fsbench.c" > "$O/fsbench-l3.c"
grep -c "L3-only scratch variant" "$O/fsbench-l3.c"
# the empty frame (no layer at all): boot + FSBENCH's own I/O, the baseline
# that is subtracted from the L3-only trace counts
sed -e 's/^\t\trun_l3(p);$/\t\t\/* empty scratch variant: run_l3(p); *\//' "$O/fsbench-l3.c" > "$O/fsbench-none.c"
grep -c "empty scratch variant" "$O/fsbench-none.c"
MINT=$SDK/libc/mintlib/include
FCF=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__ -Wno-macro-redefined)
clang "${FCF[@]}" -I"$SDK/examples/fsbench" -I"$MINT" -isystem "$RES" -c "$SDK/examples/fsbench.c" -o "$O/fsbench.o"
clang "${FCF[@]}" -I"$SDK/examples/fsbench" -I"$MINT" -isystem "$RES" -c "$O/fsbench-l3.c" -o "$O/fsbench-l3.o"
clang "${FCF[@]}" -I"$SDK/examples/fsbench" -I"$MINT" -isystem "$RES" -c "$O/fsbench-none.c" -o "$O/fsbench-none.o"
clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc -I"$SDK/include" -I"$MINT" -c "$SDK/libc/crt0.S" -o "$O/crt0.o"
for k in 1 16 32 64; do
  for v in fsbench fsbench-l3 fsbench-none; do
    "$BIN/rust-lld" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
      --build-id=none "$O/crt0.o" "$O/$v.o" "$O/libc-${k}k.a" -o "$S/explore/$v-${k}k.NCP"
  done
done
shasum -a 256 "$S/explore/"*.NCP "$O"/libc-*.a
