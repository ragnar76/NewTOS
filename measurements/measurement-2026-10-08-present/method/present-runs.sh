#!/usr/bin/env bash
# present-runs.sh <scratch root> <label> <part...> -- QEMU raspi4b runs of
# TASK/PRESENT01 (serial=on), one QEMU at a time. Adapted from
# measurement-2026-10-08-sysinfo2/method/sysinfo2-runs.sh.
#
# Card: <S>/base.img = tools/make-sd-image.sh <S>/base.img 64 (QEMU image only;
# it installs C:\Apps\PRESENT\{PRESENT.NAP,SLIDES.TXT,TEST.PNG}). Per run a
# private APFS clone (cp -c) gets C:\NEWDESK.INF = newdesk-template.inf with the
# run's "#Z" line, via mtools at the partition offset (LBA 63); the clone is
# deleted after the run, after C:\Apps\PRESENT\PRESENT.LOG (the sample file's
# @log) was copied out to ../checks/.
# Injector kernel: make-inj-input01.sh <S> <tree> k- nav -> <S>/k-kinj-nav.img
# Plain kernel (APITEST): <S>/kernel8-plain.img (shared tree, release).
# Parts:
#   vga       640x480,   nav.actions
#   hd        1920x1080, nav.actions
#   strict    640x480,   nav.actions, -append "serial=on memprot=strict"
#   hdstrict  1920x1080, nav.actions, memprot=strict
#   xga       1024x768,  nav.actions (a common projector size)
#   api       APITEST (#Z C:\APITEST.NCP), plain kernel
set -uo pipefail
S="${1:?scratch root}"; L="${2:?label}"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$S/base.img"; C="$S/cards"; mkdir -p "$C"
OFF=$((63 * 512))
WS="$(cd "$HERE/../../.." && pwd)"
CHK="$HERE/../checks"; mkdir -p "$CHK"

card() { # card <out> <autorun>
  local out=$1 auto=$2
  rm -f "$out"; cp -c "$BASE" "$out"
  sed "s|^#Z .*|#Z 01 ${auto//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mcopy -o -i "$out@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
}
pull() { # pull <card> <prefix>
  mdir -i "$1@@$OFF" ::Apps/PRESENT > "$CHK/$2-presentdir.txt" 2>&1
  mcopy -o -i "$1@@$OFF" ::Apps/PRESENT/PRESENT.LOG "$CHK/$2-PRESENT.LOG" 2>/dev/null \
    || echo "PRESENT.LOG absent" > "$CHK/$2-PRESENT.LOG"
}
nav() { # nav <part> <xr> <yr> <append>
  card "$C/$1.img" 'C:\Apps\PRESENT\PRESENT.NAP'
  XR=$2 YR=$3 APPEND="$4" bash "$HERE/run-rpi4-drive-m3.sh" "$S/k-kinj-nav.img" "$C/$1.img" "$L-$1" \
    --mark-delay 0.1 --end '\[m3\] MARK END_DESKTOP' --end-delay 1 --timeout 240
  pull "$C/$1.img" "$L-$1"
  rm -f "$C/$1.img"
}

for part in "$@"; do
  case "$part" in
  vga)      nav vga 640 480 "serial=on" ;;
  hd)       nav hd 1920 1080 "serial=on" ;;
  strict)   nav strict 640 480 "serial=on memprot=strict" ;;
  hdstrict) nav hdstrict 1920 1080 "serial=on memprot=strict" ;;
  xga)      nav xga 1024 768 "serial=on" ;;
  api)
    card "$C/api.img" 'C:\APITEST.NCP'
    mcopy -o -i "$C/api.img@@$OFF" "$WS/sdk/apps/apitest/APITEST.NCP" ::APITEST.NCP
    APPEND=serial=on bash "$HERE/run-rpi4.sh" "$S/kernel8-plain.img" "$C/api.img" "$L-api" 25:t25.png 40:t40.png
    rm -f "$C/api.img" ;;
  *) echo "part? $part"; exit 2;;
  esac
done
