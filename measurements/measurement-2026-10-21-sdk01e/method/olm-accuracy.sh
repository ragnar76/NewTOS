#!/usr/bin/env bash
# olm-accuracy.sh -- host OpenLibm (same sources/flags as sdk/compiler-rt/tests/rt-host.sh) + olm-accuracy.c
# -> ../checks/olm-accuracy.txt (informational, upstream OpenLibm behaviour)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
W="$S/olmacc"; rm -rf "$W"; mkdir -p "$W/o"; OL_TOP="$WS/sdk/libm/openlibm-0.8.8"
while IFS= read -r c; do d="${c%%/*}"; o="${d}__$(basename "${c%.c}").o"
  cc -O2 -std=c99 -fno-builtin -fno-strict-aliasing -D__BSD_VISIBLE -Wno-implicit-function-declaration \
    -I"$OL_TOP" -I"$OL_TOP/include" -I"$OL_TOP/aarch64" -I"$OL_TOP/src" -c "$OL_TOP/$c" -o "$W/o/$o" 2>/dev/null || exit 1
done < "$WS/sdk/libm/build/srcs.txt"
ar cr "$W/olh.a" "$W"/o/*.o
cc -O2 -o "$W/acc" "$HERE/olm-accuracy.c" "$W/olh.a" || exit 1
"$W/acc" | tee "$HERE/../checks/olm-accuracy.txt"
