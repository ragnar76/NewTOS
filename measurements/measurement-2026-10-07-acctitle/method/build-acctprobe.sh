#!/usr/bin/env bash
# build-acctprobe.sh <out> -- builds ACCPROBE.NAP from acctprobe.c with the
# chaintest flag set (sdk/apps/chaintest/build-chaintest.sh), gemlib + libc.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"; SDK="$WS/sdk"
OUT="${1:?out}"; T="$(mktemp -d "${TMPDIR:-/private/tmp/claude-501}"/acp.XXXXXX)"
RES="$(clang -print-resource-dir)/include"
BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
CF=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -Os -fomit-frame-pointer -fPIE
    -fno-builtin -nostdinc -funsigned-char -D__NEWTOS__ -Wno-macro-redefined)
INC=(-I"$SDK/gemlib/gemlib-0.44.0" -I"$SDK/libc/mintlib/include" -isystem "$RES")
clang "${CF[@]}" "${INC[@]}" -c "$HERE/acctprobe.c" -o "$T/acctprobe.o"
clang "${CF[@]}" -I"$SDK/include" -I"$SDK/libc/mintlib/include" -c "$SDK/libc/crt0.S" -o "$T/crt0.o"
"$BIN/rust-lld" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
  --build-id=none "$T/crt0.o" "$T/acctprobe.o" --start-group "$SDK/gemlib/gemlib.a" "$SDK/libc/libc.a" --end-group -o "$OUT"
rm -rf "$T"
echo "ACCPROBE.NAP: $(wc -c < "$OUT" | tr -d ' ') bytes, undefined: $("$BIN/llvm-nm" -u "$OUT" | grep -c . || true)"
