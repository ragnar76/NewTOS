#!/usr/bin/env bash
# COPIED from measurement-2026-10-07-fat-curcl/method/prep-card.sh for TASK/FS08
# part 4 (stdiobuf). Changes: the base card is $BASE (default <scratch>/base.img;
# here base-before.img = old program builds, base-after.img = rebuilt ones,
# both tools/make-sd-image.sh 256 MiB with SD_FSBENCH_D); two kinds added:
#   zvs    #Z C:\ZVIEW\ZVIEW.NAP (the browser run of zv-samples.actions,
#          measurement-2026-10-07-zview02: C:\ZVIEW\SAMPLES thumbnails)
#   ldg    the LDGDEMO tree from $LDG_DIR (CLIENT\LDGDEMO.NAP + CLIENT.RSC,
#          LIBS\EX_LIB.NLD + POPUP.NLD) at C:\LDGDEMO\, #Z its LDGDEMO.NAP
#          (evb-acc.actions, measurement-2026-10-07-zview02)
# FS09: kind zv2 added -- NEWDESK.INF = desk/newdesk-zv2.inf (EmuDESK, no #Z,
# window C:\ZVIEW\ restored, desk3a geometry) for the zView-twice run.
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
rm -f "$OUT"; cp -c "${BASE:-$S/base.img}" "$OUT"
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
  zvs)   inf 'C:\ZVIEW\ZVIEW.NAP' ;;
  zv2)   mcopy -o -i "$I" "$HERE/desk/newdesk-zv2.inf" ::NEWDESK.INF ;;
  ldg)   L="${LDG_DIR:?LDG_DIR = the LDGDEMO tree}"
         mmd -i "$I" ::LDGDEMO ::LDGDEMO/CLIENT ::LDGDEMO/LIBS
         mcopy -o -i "$I" "$L/CLIENT/LDGDEMO.NAP" ::LDGDEMO/CLIENT/LDGDEMO.NAP
         mcopy -o -i "$I" "$L/CLIENT/CLIENT.RSC" ::LDGDEMO/CLIENT/CLIENT.RSC
         mcopy -o -i "$I" "$L/LIBS/EX_LIB.NLD" ::LDGDEMO/LIBS/EX_LIB.NLD
         mcopy -o -i "$I" "$L/LIBS/POPUP.NLD" ::LDGDEMO/LIBS/POPUP.NLD
         inf 'C:\LDGDEMO\CLIENT\LDGDEMO.NAP' ;;
  *) echo "kind? $KIND"; exit 2 ;;
esac
echo "card $OUT ($KIND) ready"
