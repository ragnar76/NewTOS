#!/usr/bin/env bash
# make-bases-keyowner.sh <scratch root> -- ONE base card per card content
# (user rule: per run an APFS clone `cp -c`, deleted right after the run):
#   <S>/bases/base64.img    tools/make-sd-image.sh 64 MiB (QEMU image only):
#                           C:\ACC\SYSACC.NAC + C:\SYSACC2.NAC (this tree's
#                           sdk/examples/sysacc.c), QED, TeraDesk, zView ...
#   <S>/bases/ldgdemo.img, desk.img, teradesk.img, qed.img, apitest.img
#                           seed-card.sh (INPUT01/ofam group A cards, program
#                           set measurement-2026-10-06-ldg-m3/bin, NO accessory)
#   <S>/bases/desk3a.img    seed-card-a4.sh (the ARCH03 desk3a card, no accessory)
#   <S>/bases/desk3a-acc.img  desk3a.img + C:\ACC\SYSACC.NAC (this tree's build):
#                           the desk3a keyboard script with an accessory alive
#   <S>/bases/doom.img      base64.img + C:\DOOM\{DOOM.NAP,DOOM1.WAD}
set -euo pipefail
S="${1:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
B="$WS/measurements/measurement-2026-10-06-ldg-m3/bin"
A4="$WS/measurements/measurement-2026-10-06-arch03-4"
D="$S/bases"; mkdir -p "$D"
off() { python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$1"; }
"$WS/tools/make-sd-image.sh" "$D/base64.img" 64 > "$S/mkimg.log" 2>&1
DEMO="LDGDEMO/CLIENT/LDGDEMO.NAP=$B/LDGDEMO/CLIENT/LDGDEMO.NAP LDGDEMO/CLIENT/CLIENT.RSC=$B/LDGDEMO/CLIENT/CLIENT.RSC LDGDEMO/LIBS/EX_LIB.NLD=$B/LDGDEMO/LIBS/EX_LIB.NLD LDGDEMO/LIBS/POPUP.NLD=$B/LDGDEMO/LIBS/POPUP.NLD"
EXTRA="$DEMO" BIN="$B" bash "$HERE/seed-card.sh" "$D/ldgdemo.img" 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' > /dev/null
INF="$HERE/newdesk-evb-desk.inf" BIN="$B" bash "$HERE/seed-card.sh" "$D/desk.img" - > /dev/null
BIN="$B" bash "$HERE/seed-card.sh" "$D/teradesk.img" 'C:\Apps\TERADESK\DESKTOP.NAP' > /dev/null
BIN="$B" bash "$HERE/seed-card.sh" "$D/qed.img" 'C:\Apps\QED\QED.NAP' > /dev/null
BIN="$B" bash "$HERE/seed-card.sh" "$D/apitest.img" 'C:\APITEST.NCP' > /dev/null
BIN="$A4/bin" INF="$HERE/desk3a/newdesk-desk3a.inf" \
  EXTRA="D3A/SUB3A/INSIDE.TXT=$HERE/desk3a/INSIDE.TXT D3A/NOTE.TXT=$HERE/desk3a/NOTE.TXT D3A/HELLO.NCP=$A4/bin/HELLO.NCP" \
  bash "$HERE/seed-card-a4.sh" "$D/desk3a.img" - > /dev/null
cp -c "$D/desk3a.img" "$D/desk3a-acc.img"
O=$(off "$D/desk3a-acc.img")
mmd -i "$D/desk3a-acc.img@@$O" ::ACC 2>/dev/null || true
OB=$(off "$D/base64.img")
mcopy -o -i "$D/base64.img@@$OB" ::ACC/SYSACC.NAC "$S/SYSACC-base64.NAC"
mcopy -o -i "$D/desk3a-acc.img@@$O" "$S/SYSACC-base64.NAC" ::ACC/SYSACC.NAC
cp -c "$D/base64.img" "$D/doom.img"
mmd -i "$D/doom.img@@$OB" ::DOOM 2>/dev/null || true
mcopy -o -i "$D/doom.img@@$OB" "$WS/sdk/apps/doom/build/DOOM.NAP" ::DOOM/DOOM.NAP
mcopy -o -i "$D/doom.img@@$OB" "$WS/Applications/doom-wad/doom1.wad" ::DOOM/DOOM1.WAD
shasum -a 256 "$D"/*.img "$S/SYSACC-base64.NAC"
