#!/usr/bin/env bash
# jpgref.sh <in.jpg> <out.rgb> <scratch dir> -- builds the pinned IJG 9f for
# the host (sdk/libjpeg/jpeg-9f, the target build's own source list
# sdk/libjpeg/build/compiled.txt and config overlay, as
# sdk/examples/libtest/make-ref.sh does) and runs jpgref.c on <in.jpg>.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"; J="$WS/sdk/libjpeg/jpeg-9f"; T="$3/jpgref"
mkdir -p "$T"; rm -f "$T"/*.o
while IFS= read -r c; do cc -O2 -w -I"$J" -c "$J/$c" -o "$T/$(basename "${c%.c}").o"; done < "$WS/sdk/libjpeg/build/compiled.txt"
cc -O2 -I"$J" "$HERE/jpgref.c" "$T"/*.o -o "$T/jpgref"
"$T/jpgref" "$1" "$2"
