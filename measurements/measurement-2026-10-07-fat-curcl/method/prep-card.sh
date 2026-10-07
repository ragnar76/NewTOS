#!/usr/bin/env bash
# prep-card.sh <scratch> <out.img> <kind> -- an APFS clone of <scratch>/base.img
# (prep-base.sh: tools/make-sd-image.sh 256 MiB, C: NEWTOS + D: NEWTOS1, QEMU
# image only) with the files of one GUI write run, via mtools at C:'s offset:
#   desk   C:\D3A\SUB3A\INSIDE.TXT, C:\D3A\BIG.DAT (300003 B, seed 77),
#          C:\D3A\NOTE.TXT; NEWDESK.INF = desk/newdesk-deskw.inf (window C:\D3A\)
#   tera   the same files; NEWDESK.INF = acctitle template, #Z TeraDesk
#   qed    C:\D3A\NOTE.TXT; NEWDESK.INF #Z QED
#   zview  ZVLAUNCH (#Z, measurement-2026-10-06-zview-m4/bin) + ZVARG.TXT ->
#          C:\PHOTO\BIG.JPG (2 273 801 B, measurement-2026-10-07-fs08/bin/
#          pi-package, sha256 26ab5f8a...)
set -euo pipefail
S="$1"; OUT="$2"; KIND="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
rm -f "$OUT"; cp -c "$S/base.img" "$OUT"
OFF=$(python3 -c "import struct,sys;d=open(sys.argv[1],'rb').read(512);print(struct.unpack('<I',d[0x1C6:0x1CA])[0]*512)" "$OUT")
I="$OUT@@$OFF"
files() {
  python3 "$HERE/gen-pattern.py" 77 300003 "$S/BIG.DAT"
  mmd -i "$I" ::D3A ::D3A/SUB3A
  mcopy -o -i "$I" "$HERE/desk/INSIDE.TXT" ::D3A/SUB3A/INSIDE.TXT
  mcopy -o -i "$I" "$S/BIG.DAT" ::D3A/BIG.DAT
  mcopy -o -i "$I" "$HERE/desk/NOTE.TXT" ::D3A/NOTE.TXT
}
inf() { # inf <autorun>
  sed "s|^#Z .*|#Z 01 ${1//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mcopy -o -i "$I" "$S/NEWDESK.INF" ::NEWDESK.INF
}
case "$KIND" in
  desk)  files; mcopy -o -i "$I" "$HERE/desk/newdesk-deskw.inf" ::NEWDESK.INF ;;
  tera)  files; inf 'C:\Apps\TERADESK\DESKTOP.NAP' ;;
  qed)   mmd -i "$I" ::D3A; mcopy -o -i "$I" "$HERE/desk/NOTE.TXT" ::D3A/NOTE.TXT; inf 'C:\Apps\QED\QED.NAP' ;;
  zview) inf 'C:\ZVLAUNCH.NAP'
         mcopy -o -i "$I" "$WS/measurements/measurement-2026-10-06-zview-m4/bin/ZVLAUNCH.NAP" ::ZVLAUNCH.NAP
         mmd -i "$I" ::PHOTO
         mcopy -o -i "$I" "$WS/measurements/measurement-2026-10-07-fs08/bin/pi-package/BIG.JPG" ::PHOTO/BIG.JPG
         printf '%s\r\n' 'C:\PHOTO\BIG.JPG' > "$S/ZVARG.TXT"
         mcopy -o -i "$I" "$S/ZVARG.TXT" ::ZVARG.TXT ;;
  *) echo "kind? $KIND"; exit 2 ;;
esac
echo "card $OUT ($KIND) ready"
