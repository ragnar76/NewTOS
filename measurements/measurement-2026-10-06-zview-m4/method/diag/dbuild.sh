#!/bin/bash
# scratch: rebuild ZVIEW.NAP with an instrumented copy of one zView file
# usage: dbuild.sh <relpath under zview/> <modified file> <out NAP>
set -e
SDK=/Volumes/Daten/Development/PiRusTOS/sdk; ZD=$SDK/zview; TREE=$ZD/build/tree; OBJ=$ZD/build/obj
RES="$(clang -print-resource-dir)/include"; BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
BASE=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc -fsigned-char -fgnu89-inline -D__NEWTOS__ -DHAVE_INTS_DEFINED -include $ZD/include/zview-ints.h -Wno-macro-redefined -w)
INCS=(-I$TREE/common -I$TREE/zview -I$TREE/zview/$(dirname $1) -I$TREE/ztask -I$SDK/windom/include -I$SDK/ldg/ldg-r133/include -I$SDK/libpng/libpng-1.6.59 -I$SDK/zlib/zlib-1.3.2 -I$SDK/gemlib/gemlib-0.44.0 -I$SDK/libc/mintlib/include -I$SDK/libm/include -I$SDK/libm/openlibm-0.8.8/include -isystem $RES)
T=$(mktemp -d); cp $OBJ/zview/*.o $T/
o=$T/$(echo ${1%.c} | tr / _).o
clang "${BASE[@]}" "${INCS[@]}" -c $2 -o $o
$BIN/rust-lld -flavor gnu -T $SDK/newtos.ld -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none $OBJ/crt0.o $T/*.o --start-group $SDK/windom/libwindom1.a $SDK/ldg/libldg.a $SDK/gemlib/gemlib.a $SDK/libpng/libpng.a $SDK/zlib/libz.a $SDK/libm/libm.a $SDK/libc/libc.a --end-group -o $3
rm -rf $T; ls -la $3
