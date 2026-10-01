#!/usr/bin/env bash
# reconstitute-before.sh <ws> <out-dir>
# The pre-T04 DESKTOP.NAP without touching the tree: TeraDesk's own objects
# (sdk/apps/teradesk/build/obj, crt0.o) linked against a COPY of gemlib.a in
# which the one changed member TeraDesk links (a_wind_get.o, why.txt) is
# replaced by the pristine tarball a_wind_get.c compiled with HEAD's overlay
# headers (git show, never a checkout). Disassembly diff against the T04
# DESKTOP.NAP: one instruction (mov w9,#4 -> #6, mt_wind_get's IN_LEN for
# WF_NAME/WF_INFO).
set -euo pipefail
WS="$1"; OUT="$2"; mkdir -p "$OUT/hdr"
SDK="$WS/sdk"; G="$SDK/gemlib/gemlib-0.44.0"
SYSROOT="$(rustc --print sysroot)"; BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
RES="$(clang -print-resource-dir)/include"
tmp="$(mktemp -d)"; tar xf "$SDK/gemlib/gemlib-0.44.0.tar.xz" -C "$tmp"
cp "$tmp/gemlib-0.44.0/a_wind_get.c" "$OUT/hdr/"
( cd "$WS" && git show HEAD:sdk/gemlib/overlay/gem_aesP.h > "$OUT/hdr/gem_aesP.h" && git show HEAD:sdk/gemlib/overlay/gem_vdiP.h > "$OUT/hdr/gem_vdiP.h" )
cp "$SDK/gemlib/gemlib.a" "$OUT/gemlib.a"
clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fno-pic -fno-builtin -nostdinc -D__NEWTOS__ \
  -Wno-macro-redefined -I"$OUT/hdr" -I"$G" -I"$SDK/libc/mintlib/include" -isystem "$RES" \
  -c "$OUT/hdr/a_wind_get.c" -o "$OUT/a_wind_get.o"
"$BIN/llvm-ar" r "$OUT/gemlib.a" "$OUT/a_wind_get.o"
"$BIN/rust-lld" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 --build-id=none \
  "$SDK/apps/teradesk/build/crt0.o" "$SDK/apps/teradesk/build/obj"/*.o \
  --start-group "$OUT/gemlib.a" "$SDK/libc/libc.a" --end-group -o "$OUT/DESKTOP.NAP"
shasum -a 256 "$OUT/DESKTOP.NAP"
