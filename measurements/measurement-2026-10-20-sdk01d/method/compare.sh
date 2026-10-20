#!/usr/bin/env bash
# compare.sh -- the existing build scripts vs sdk/bin/newtos-cc on the same sources: each pair is
# compared with cmp (byte identity) and its sha256 printed. Script outputs land in their usual
# (untracked) build directories and are copied to $S/cmp; the wrapper builds go to $S/cmp directly.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
C="$S/cmp"; rm -rf "$C"; mkdir -p "$C"; cd "$WS"
NCC=sdk/bin/newtos-cc
pair() { if cmp "$C/$1" "$C/$2"; then echo "IDENTICAL  $1 == $2"; else echo "DIFFERENT  $1 != $2"; fi
         shasum -a 256 "$C/$1" "$C/$2" | sed "s|$C/||"; }

echo "== 1 sdk/build.sh (freestanding) vs newtos-cc -bare: sdk/examples/hello.c"
sdk/build.sh sdk/examples/hello.c "$C/HELLO2-s.NCP" > "$C/build-sh.log" 2>&1 || echo "build.sh FAILED"
$NCC -bare sdk/examples/hello.c -o "$C/HELLO2-w.NCP"
pair HELLO2-s.NCP HELLO2-w.NCP

echo "== 2 sdk/apps/gemtest/build-gemtest.sh vs newtos-cc -nap: TESTGEM.NAP (gemlib + libc)"
sdk/apps/gemtest/build-gemtest.sh "$C/TESTGEM-s.NAP" > "$C/gemtest.log" 2>&1 || echo "build-gemtest FAILED"
$NCC -nap -Os -fomit-frame-pointer -funsigned-char -D_TOS_ONLY sdk/apps/gemtest/gemtest.c -o "$C/TESTGEM-w.NAP"
pair TESTGEM-s.NAP TESTGEM-w.NAP

echo "== 3 sdk/apps/build-coremark.sh vs newtos-cc -ncp: CORE.NCP / COREVAL.NCP (6 sources, libc)"
sdk/apps/build-coremark.sh > "$C/coremark.log" 2>&1 || echo "build-coremark FAILED"
cp sdk/apps/coremark/build/CORE.NCP "$C/CORE-s.NCP"; cp sdk/apps/coremark/build/COREVAL.NCP "$C/COREVAL-s.NCP"
CM=sdk/apps/coremark; P=sdk/overlays/coremark/newtos
F="-target aarch64-unknown-none-elf -O2 -fomit-frame-pointer -ffreestanding -fPIE -fno-builtin"
# object order = the script's glob "$OBJ/$tag"/core_*.o (core_portme between core_matrix and core_state)
for R in PERFORMANCE:CORE VALIDATION:COREVAL; do
  $NCC -ncp -O2 -fomit-frame-pointer -D_GNU_SOURCE "-DFLAGS_STR=\"$F\"" -DITERATIONS=0 -D${R%%:*}_RUN=1 \
    -I$P -I$CM $CM/core_list_join.c $CM/core_main.c $CM/core_matrix.c $P/core_portme.c \
    $CM/core_state.c $CM/core_util.c -o "$C/${R##*:}-w.NCP"
  pair "${R##*:}-s.NCP" "${R##*:}-w.NCP"
done

echo "== 4 sdk/ldg/build-ldg-demo.sh vs newtos-cc -nld / -lldg: EX_LIB.NLD (libshare first) + LDGTEST.NCP"
sdk/ldg/build-ldg-demo.sh > "$C/ldgdemo.log" 2>&1 || echo "build-ldg-demo FAILED"
cp sdk/ldg/build/EX_LIB.NLD "$C/EX_LIB-s.NLD"; cp sdk/ldg/build/LDGTEST.NCP "$C/LDGTEST-s.NCP"
T=sdk/ldg/ldg-r133
$NCC -nld -lm -funsigned-char -I$T/demo/libs/ldg $T/demo/libs/ex_lib.c -o "$C/EX_LIB-w.NLD"
pair EX_LIB-s.NLD EX_LIB-w.NLD
$NCC -ncp -lldg -lm -funsigned-char -I$T/demo/libs/ldg sdk/examples/ldgtest.c $T/demo/libs/exlibbnd.c -o "$C/LDGTEST-w.NCP"
pair LDGTEST-s.NCP LDGTEST-w.NCP
