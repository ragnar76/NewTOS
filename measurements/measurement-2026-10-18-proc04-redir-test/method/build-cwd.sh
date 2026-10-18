#!/usr/bin/env bash
# build-cwd.sh <outdir> -- cwdprobe.c (Tester scaffolding) built like build-p4.sh; roles CWDR/CWDA/CWDB/CWDT (one source, tag).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"
SDK="$WS/sdk"; OUT="${1:?outdir}"; GEM="$SDK/gemlib"; LIBC="$SDK/libc"; GEMTOP="$GEM/gemlib-0.44.0"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"; MINT="$LIBC/mintlib/include"; CLANG="${CLANG:-clang}"
RES="$($CLANG -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"; LLD="$BIN/rust-lld"
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os -fomit-frame-pointer -fPIE -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__ -D_TOS_ONLY -Wno-macro-redefined)
INCS=(-I"$GEMTOP" -I"$MINT" -isystem "$RES" -I"$HERE")
W="$(mktemp -d "$OUT/w.XXXX")"
for t in CWDR CWDA CWDB CWDT; do
  "$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$MINT" -c "$LIBC/crt0.S" -o "$W/crt0.o" || exit 1
  "$CLANG" "${CFLAGS[@]}" "${INCS[@]}" "-DCWD_TAG=\"$t\"" -c "$HERE/cwdprobe.c" -o "$W/c.o" || exit 1
  "$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
    "$W/crt0.o" "$W/c.o" --start-group "$GEM/gemlib.a" "$LIBC/libc.a" --end-group -o "$OUT/$t.NAP" || exit 1
  echo "built $t.NAP"
done
rm -rf "$W"
