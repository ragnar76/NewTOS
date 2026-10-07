#!/usr/bin/env bash
# prep-doom.sh <base.img> <out.img> <DOOM.NAP> -- an APFS clone of <base.img>
# (tools/make-sd-image.sh, C: + D:) with C:\DOOM\DOOM.NAP (the given build),
# C:\DOOM\DOOM1.WAD (Applications/doom-wad/doom1.wad, the shareware WAD the
# earlier Doom runs used) and NEWDESK.INF = newdesk-template.inf, whose #Z line
# autoruns C:\DOOM\DOOM.NAP; via mtools at C:'s offset.
set -euo pipefail
BASE="$1"; OUT="$2"; NAP="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
rm -f "$OUT"; cp -c "$BASE" "$OUT"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$OUT")
I="$OUT@@$OFF"
mmd -i "$I" ::DOOM
mcopy -o -i "$I" "$NAP" ::DOOM/DOOM.NAP
mcopy -o -i "$I" "$WS/Applications/doom-wad/doom1.wad" ::DOOM/DOOM1.WAD
mcopy -o -i "$I" "$HERE/newdesk-template.inf" ::NEWDESK.INF
echo "card $OUT ready ($(shasum -a 256 "$NAP" | cut -c1-16) DOOM.NAP)"
