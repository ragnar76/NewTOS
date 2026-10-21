#!/usr/bin/env bash
# build-probe.sh -- LDP.NCP from method/ldp.c with the working-tree sdk/bin/newtos-cc -lm (shipped libm.a, libc.a,
# libclang_rt.builtins.a); no -I .../openlibm-0.8.8/src (SDK01 E needed it for <fenv.h>). Also: the same source against the
# archives as found at HEAD ($S/before/) -- link fails (the `l' functions are missing). Output: $S/prog/LDP.NCP (+ sha256).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; mkdir -p "$S/prog"
"$WS/sdk/bin/newtos-cc" -ncp -lm -Wall -I"$HERE" -Wl,-Map="$S/prog/ldp.map" "$HERE/ldp.c" -o "$S/prog/LDP.NCP" || exit 1
BIN="$(cd "$WS/sdk" && . ./toolchain.sh && newtos_rust_bindir)"
echo "LDP.NCP $(wc -c < "$S/prog/LDP.NCP" | tr -d ' ') bytes, undefined: $("$BIN/llvm-nm" -u "$S/prog/LDP.NCP" | grep -c .)"
echo "libm members linked: $(grep -o 'libm.a([^)]*)' "$S/prog/ldp.map" | sort -u | wc -l | tr -d ' '), ld128 among them: $(grep -o 'libm.a(ld128__[^)]*)' "$S/prog/ldp.map" | sort -u | sed 's/libm.a(ld128__//;s/\.o)//' | tr '\n' ' ')"
"$WS/sdk/bin/newtos-cc" -c "$WS/sdk/libc/crt0.S" -o "$S/prog/crt0.o" && "$WS/sdk/bin/newtos-cc" -ncp -lm -I"$HERE" -c "$HERE/ldp.c" -o "$S/prog/ldp.o" || exit 1
"$BIN/rust-lld" -flavor gnu -T "$WS/sdk/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 --build-id=none \
  "$S/prog/crt0.o" "$S/prog/ldp.o" --start-group "$S/before/libm.a" "$S/before/libc.a" "$S/before/libclang_rt.builtins.a" --end-group \
  -o "$S/prog/LDP-before.NCP" 2> "$S/prog/before.err" && echo "HEAD archives: links (unexpected)" ||
  echo "HEAD archives: link fails, undefined: $(grep -o 'undefined symbol: [^ ]*' "$S/prog/before.err" | sort -u | sed 's/undefined symbol: //' | tr '\n' ' ')"
shasum -a 256 "$S/prog/LDP.NCP"
