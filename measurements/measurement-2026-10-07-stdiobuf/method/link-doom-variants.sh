#!/usr/bin/env bash
# link-doom-variants.sh <scratch> -- exploration (TASK/FS08 part 4): relink
# DOOM.NAP from the objects sdk/apps/build-doom.sh left in
# sdk/apps/doom/build/obj (+ its crt0.o), with the link line of
# build-doom.sh, once per libc variant of build-variants.sh
# (<scratch>/explore/o/libc-{1,16,32,64}k.a). DOOM is the random-access stdio
# user: every lump is fseek + fread (doomgeneric w_file_stdc.c W_StdC_Read).
# libc-1k must reproduce the DOOM.NAP that was in the tree before the change.
set -euo pipefail
S="$1"; SDK=/Volumes/Daten/Development/PiRusTOS/sdk; B=$SDK/apps/doom/build
LLD="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/rust-lld"
for k in 1 16 32 64; do
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
    --build-id=none "$B/crt0.o" "$B/obj"/*.o \
    --start-group "$SDK/gemlib/gemlib.a" "$SDK/libm/libm.a" "$S/explore/o/libc-${k}k.a" --end-group \
    -o "$S/explore/DOOM-${k}k.NAP"
done
shasum -a 256 "$S/explore/"DOOM-*.NAP "$S/before/DOOM.NAP"
