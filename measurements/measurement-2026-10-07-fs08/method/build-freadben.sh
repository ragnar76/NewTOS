#!/usr/bin/env bash
# build-freadben.sh <workspace> <out.NCP> -- freadben.c against the existing
# sdk/libc/libc.a (not rebuilt), crt0 + newtos.ld, like build-zvlaunch.sh.
set -euo pipefail
WS="${1:?workspace}"; OUT="${2:?out}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK="$WS/sdk"; T="$(mktemp -d "${TMPDIR:-/private/tmp}/frb.XXXXXX")"
RES="$(clang -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
CF=(${EXTRA_CFLAGS:-} -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fPIE -fno-builtin -nostdinc
    -funsigned-char -D__NEWTOS__ -Wno-macro-redefined)
clang "${CF[@]}" -I"$SDK/libc/mintlib/include" -isystem "$RES" -c "$HERE/freadben.c" -o "$T/freadben.o"
clang "${CF[@]}" -I"$SDK/include" -I"$SDK/libc/mintlib/include" -c "$SDK/libc/crt0.S" -o "$T/crt0.o"
"$BIN/rust-lld" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
  --build-id=none "$T/crt0.o" "$T/freadben.o" "$SDK/libc/libc.a" -o "$OUT"
echo "FREADBEN.NCP: $(wc -c < "$OUT" | tr -d ' ') bytes, undefined: $("$BIN/llvm-nm" -u "$OUT" | grep -c . || true)"
rm -rf "$T"
