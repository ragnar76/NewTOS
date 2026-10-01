#!/usr/bin/env bash
# build.sh -- two host binaries:
#   settest-before : device 1 = pristine src/set.c, device 2 = pristine src/set.c
#                    (proves the harness is RED before the overlay exists)
#   settest-after  : device 1 = pristine src/set.c, device 2 = sdk/overlays/qed/set.c
# QED's own compile flags for these files: -funsigned-char -O2 (build-qed.sh:91).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CC=${CC:-clang}
F="-funsigned-char -O2 -Wall -Wextra -I$HERE -I$HERE/shim"
RENAME_O="-Dsetfree=o_setfree -Dsetmax=o_setmax -Dsetmin=o_setmin -Dsetcpy=o_setcpy \
 -Dsetall=o_setall -Dsetclr=o_setclr -Dsetnot=o_setnot -Dsetand=o_setand -Dsetor=o_setor \
 -Dsetxor=o_setxor -Dsetincl=o_setincl -Dsetexcl=o_setexcl -Dsetchg=o_setchg -Dsetin=o_setin \
 -Dsetcmp=o_setcmp -Dsetcard=o_setcard -Dstr2set=o_str2set"
RENAME_N="${RENAME_O//o_/n_}"
$CC $F $RENAME_O -c "$HERE/orig/set.c" -o "$HERE/orig-set.o"
$CC $F $RENAME_N -c "$HERE/orig/set.c" -o "$HERE/orig-set-as-n.o"
$CC $F $RENAME_N -c "$HERE/ovl/set.c"  -o "$HERE/ovl-set.o"
$CC $F -c "$HERE/m68kmodel.c" -o "$HERE/m68kmodel.o"
$CC $F -c "$HERE/harness.c" -o "$HERE/harness-after.o"
$CC $F -DDEV2_NAME='"src/set.c (pristine, second copy)"' -c "$HERE/harness.c" -o "$HERE/harness-before.o"
$CC "$HERE/harness-before.o" "$HERE/m68kmodel.o" "$HERE/orig-set.o" "$HERE/orig-set-as-n.o" -o "$HERE/settest-before"
$CC "$HERE/harness-after.o" "$HERE/m68kmodel.o" "$HERE/orig-set.o" "$HERE/ovl-set.o" -o "$HERE/settest-after"
echo "built settest-before / settest-after"
