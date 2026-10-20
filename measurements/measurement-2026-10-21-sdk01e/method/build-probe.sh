#!/usr/bin/env bash
# build-probe.sh -- RTP.NCP from method/rtp.c with the working-tree sdk/bin/newtos-cc -lm (shipped libm.a, libc.a,
# libclang_rt.builtins.a); also proves the HEAD wrapper cannot link it. Output: $S/prog/RTP.NCP (+ sha256).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; mkdir -p "$S/prog"
# -I .../openlibm-0.8.8/src: <fenv.h> through newtos-cc -lm does not compile otherwise -- OpenLibm's
# openlibm_fenv_aarch64.h includes "cdefs-compat.h", which lives in src/, not include/ (pre-existing SDK gap,
# outside SDK01 E; recorded in RUNINFO).
FENVFIX=-I"$WS/sdk/libm/openlibm-0.8.8/src"
"$WS/sdk/bin/newtos-cc" -ncp -lm $FENVFIX -Wl,-Map="$S/prog/rtp.map" "$HERE/rtp.c" -o "$S/prog/RTP.NCP" || exit 1
BIN="$(cd "$WS/sdk" && . ./toolchain.sh && newtos_rust_bindir)"
echo "RTP.NCP $(wc -c < "$S/prog/RTP.NCP" | tr -d ' ') bytes, undefined: $("$BIN/llvm-nm" -u "$S/prog/RTP.NCP" | grep -c .)"
echo "builtins members linked: $(grep -o 'libclang_rt.builtins.a([^)]*)' "$S/prog/rtp.map" | sort -u | sed 's/.*(lib_builtins_//;s/\.o)//' | tr '\n' ' ')"
git -C "$WS" show "$HEADREV:sdk/bin/newtos-cc" > "$S/prog/newtos-cc-head"; chmod +x "$S/prog/newtos-cc-head"
if NEWTOS_SDK="$WS/sdk" "$S/prog/newtos-cc-head" -ncp -lm $FENVFIX "$HERE/rtp.c" -o "$S/prog/RTP-head.NCP" 2> "$S/prog/head.err"; then
  echo "HEAD newtos-cc: links (unexpected)"
else
  echo "HEAD newtos-cc: link fails, undefined: $(grep -o 'undefined symbol: [^ ]*' "$S/prog/head.err" | sort -u | sed 's/undefined symbol: //' | tr '\n' ' ')"
fi
shasum -a 256 "$S/prog/RTP.NCP"
