#!/usr/bin/env bash
# build-m68k.sh -- the probe for the reference: VT52A.TOS / VT52B.TOS (screens 1 / 2) and VT52C.TOS (screen 1 with
# Cursconf(3), blink check) with the cross-mint gcc 7.5 / mintlib, into the Hatari GEMDOS drive $S/hd.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
CC=/opt/cross-mint/bin/m68k-atari-mint-gcc; mkdir -p "$S/hd"
$CC -O2 -Wall "$HERE/vt52prb1.c" -o "$S/hd/VT52A.TOS"
$CC -O2 -Wall "$HERE/vt52prb2.c" -o "$S/hd/VT52B.TOS"
sed 's/cursconf(2, 0);/cursconf(3, 0);/' "$HERE/vt52prb.c" > "$S/vt52prb-blink.c"
printf '#define SCREEN 1\n#include "%s"\n' "$S/vt52prb-blink.c" > "$S/blink1-m68k.c"
$CC -O2 -Wall "$S/blink1-m68k.c" -o "$S/hd/VT52C.TOS"
shasum -a 256 "$S"/hd/*.TOS
