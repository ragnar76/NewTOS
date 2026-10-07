#!/usr/bin/env bash
# build-oldexec.sh <out> -- a DELIBERATE pre-0d ET_EXEC program for the
# refusal test: sdk/examples/hello.c with the pre-0d crt0/linker script
# (method/{crt0-pre0d.S,newtos-pre0d.ld} = git show HEAD:sdk/...) and the
# pre-0d flags (-fno-pic, -no-pie). The 0d loader must refuse it with EPLFMT.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
OUT="${1:?}"
W="$(mktemp -d "${TMPDIR:-/tmp}/oldexec.XXXXXX")"
CF=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 -fno-stack-protector -fno-pic -fno-builtin -I"$WS/sdk/include")
clang "${CF[@]}" -c "$HERE/crt0-pre0d.S" -o "$W/crt0.o"
clang "${CF[@]}" -c "$WS/sdk/examples/hello.c" -o "$W/app.o"
rust-lld -flavor gnu -T "$HERE/newtos-pre0d.ld" -no-pie -z max-page-size=0x1000 --build-id=none "$W/crt0.o" "$W/app.o" -o "$OUT"
rm -rf "$W"
