#!/usr/bin/env bash
# before-host.sh -- the same requests (before-req.txt) against before-elf.c linked with the archives as found at HEAD ($S/before/)
# and with the SDK01 J archives, executed on the host by sdk/libc/tests/ldbl-host.c (shipped machine code, no x18 rebuild: a
# handful of calls). Output: ../checks/before-after-host.txt. Plus: <fenv.h> through newtos-cc -lm with sdk/libm/include as at
# HEAD (no cdefs-compat.h adapter) and as now.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; W="$S/bh"; mkdir -p "$W"; SDK="$WS/sdk"
. "$SDK/toolchain.sh"; LLD="$(newtos_rust_bindir)/rust-lld"
sed 's/^\(    \)\.init_array : {/\1. = ALIGN(0x4000);\
\1.init_array : {/' "$SDK/newtos.ld" > "$W/host.ld"
"$SDK/bin/newtos-cc" -c "$SDK/libc/crt0.S" -o "$W/crt0.o"; "$SDK/bin/newtos-cc" -c "$HERE/before-elf.c" -o "$W/b.o"
cc -O1 -w -o "$W/ldbl-host" "$SDK/libc/tests/ldbl-host.c"
O="$HERE/../checks/before-after-host.txt"; : > "$O"
for v in before after; do
  if [ $v = before ]; then A=("$S/before/libc.a" "$S/before/libclang_rt.builtins.a"); else A=("$SDK/libc/libc.a" "$SDK/compiler-rt/libclang_rt.builtins.a"); fi
  "$LLD" -flavor gnu -T "$W/host.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
    "$W/crt0.o" "$W/b.o" --start-group "${A[@]}" --end-group -o "$W/B-$v.elf" || exit 1
  echo "== $v (libc.a $(shasum -a 256 "${A[0]}" | cut -c1-16))" >> "$O"
  paste "$HERE/before-req.txt" <("$W/ldbl-host" "$W/B-$v.elf" < "$HERE/before-req.txt") | sed 's/\t/  ->  /2' >> "$O"
done
printf '#include <fenv.h>\nint main(void){ return fegetround(); }\n' > "$W/f.c"
mkdir -p "$W/headinc"; git -C "$WS" archive "$HEADREV" sdk/libm/include | tar -x -C "$W/headinc"
for v in before after; do
  if [ $v = before ]; then INC="$W/headinc/sdk/libm/include"; else INC="$SDK/libm/include"; fi
  "$SDK/bin/newtos-cc" -bare -c -I"$INC" -I"$SDK/libm/openlibm-0.8.8/include" -I"$SDK/libc/mintlib/include" "$W/f.c" -o "$W/f.o" 2> "$W/f-$v.err" \
    && echo "fenv.h ($v): compiles" >> "$O" || echo "fenv.h ($v): $(grep -m1 error "$W/f-$v.err")" >> "$O"
done
"$SDK/bin/newtos-cc" -lm -c "$W/f.c" -o "$W/f2.o" 2>/dev/null && echo "fenv.h via newtos-cc -lm (now): compiles" >> "$O" || echo "fenv.h via newtos-cc -lm (now): FAILS" >> "$O"
cat "$O"
