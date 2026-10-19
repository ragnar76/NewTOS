#!/usr/bin/env bash
# build-probes.sh <outdir> -- programs of this run, linked against the CURRENT sdk/libc/libc.a (built by
# sdk/libc/build-libc.sh with the SDK01 A change):
#   SDKP.NCP / SDKK.NCP  method/sdkp.c (parent / -DKID child), libc + gemlib group, link line of
#                        measurement-2026-10-18-proc04-fdelete/method/build-fd.sh
#   APITEST.NCP          sdk/apps/apitest/build-apitest.sh (= build-libc.sh with apitest.c as client)
#   PTERMCH.NCP, DTACHILD.NCP  sdk/build.sh (APITEST T33 / T29 children; recipe of
#                        measurement-2026-10-18-con01-test/method/regress/api-build.sh)
# Writes only <outdir> (and sdk/libc/build/, the libc build dir build-apitest.sh uses).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
SDK="$WS/sdk"; OUT="${1:?outdir}"
GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
[ -f "$GEM/gemlib.a" ] && [ -f "$LIBC/libc.a" ] || { echo "gemlib.a/libc.a missing (build them first)"; exit 1; }
MINT="$LIBC/mintlib/include"; CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"; LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$HERE")
W="$(mktemp -d "$OUT/w.XXXX")"
build() { local out="$1"; shift
  "$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "$@" -c "$HERE/sdkp.c" -o "$W/p.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
    "$W/crt0.o" "$W/p.o" --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$out" || exit 1
  echo "built $out"; }
build SDKP.NCP
build SDKK.NCP -DKID
rm -rf "$W"
cd "$WS"
sdk/build.sh sdk/examples/ptermch.c "$OUT/PTERMCH.NCP" > "$OUT/ptermch-build.log" 2>&1 || { echo "ptermch build failed"; exit 1; }
sdk/build.sh sdk/examples/dtachild.c "$OUT/DTACHILD.NCP" > "$OUT/dtachild-build.log" 2>&1 || { echo "dtachild build failed"; exit 1; }
sdk/apps/apitest/build-apitest.sh "$OUT/APITEST.NCP" > "$OUT/apitest-build.log" 2>&1 || { echo "apitest build failed"; tail -5 "$OUT/apitest-build.log"; exit 1; }
shasum -a 256 "$OUT"/*.NCP
