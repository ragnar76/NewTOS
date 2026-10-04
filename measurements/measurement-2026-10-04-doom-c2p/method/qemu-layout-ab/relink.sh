#!/usr/bin/env bash
# relink.sh <platform.c> <out.nap> : DOOM.NAP (DOOM_STATS=5) with another platform file, objects of the play build
set -euo pipefail
WS=/Volumes/Daten/Development/PiRusTOS; SDK=$WS/sdk; S=/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/b705aa1b-c171-48ba-ac49-a9f08dec4fa1/scratchpad
B=$S/qemu/doom-play; O=$S/ab/obj-$(basename $2 .nap); rm -rf $O; mkdir -p $O; cp $B/obj/*.o $O/
RES="$(clang -print-resource-dir)/include"; DGSRC=$WS/Applications/doomgeneric/doomgeneric; MINT=$SDK/libc/mintlib/include
LLD=$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/rust-lld
clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fno-pic -fno-builtin -nostdinc -D__NEWTOS__ -DCMAP256 -DDOOMGENERIC_RESX=320 -DDOOMGENERIC_RESY=200 -Wno-macro-redefined -Wall -Wextra '-DDOOM_GEM_ARGS="-mb","2"' -DDOOM_GEM_STATS=5 -I$DGSRC -I$SDK/gemlib/gemlib-0.44.0 -I$MINT -isystem $RES -c $1 -o $O/doomgeneric_gem.o
$LLD -flavor gnu -T $SDK/newtos.ld -no-pie -z max-page-size=0x1000 --build-id=none $B/crt0.o $O/*.o --start-group $SDK/gemlib/gemlib.a $SDK/libm/libm.a $SDK/libc/libc.a --end-group -o $2
