#!/usr/bin/env bash
# zv-runs.sh <scratch root> <part...> -- the QEMU raspi4b runs of TASK/ZVIEW-M4
# (serial=on). One QEMU at a time.
# K1 run (measurement-2026-10-07-k1): copied from measurement-2026-10-06-zview-m4
# with two stated changes -- the kernel is the K1 working tree's (regr/
# make-kernels.sh -> <S>/kernel8-evb.img, copied to <S>/kernel8.img), and the
# programs this script copies from ../bin (ZVLAUNCH.NAP) come from that run's
# committed bin/ (ZB below), since this run keeps no binaries of its own.
#
# Card: <S>/base.img = tools/make-sd-image.sh <S>/base.img 64 (QEMU image only;
# it installs C:\ZVIEW\ with CODECS\, ICONS\, SAMPLES\ from sdk/zview/build).
# Per run a private copy gets, via mtools at the partition offset (LBA 63):
#   C:\NEWDESK.INF   newdesk-template.inf with "#Z 01 <autorun>@"
#   C:\ZVLAUNCH.NAP  ../bin/ZVLAUNCH.NAP (build-zvlaunch.sh)
#   C:\ZVARG.TXT     zView's argument, e.g. C:\ZVIEW\SAMPLES\T24.BMP
#   C:\APITEST.NCP   sdk/apps/apitest/APITEST.NCP (regression part only)
# Kernel: <S>/kernel8.img = the K1 working tree's board-rpi4 release kernel.
# Parts:
#   probe <name>         one sample, screendumps at 15/25/35/50 s
#   plprobe              PLPROBE.NAP as C:\ZVIEW\ZVIEW.NAP (diagnosis)
#   alt <nap> <s> <tag>  a scratch diagnostic ZVIEW.NAP build, sample <s>
#   diag <nap> <mp> [s]  every sample (or the named) with a diagnostic build
#   photo <nap> <mp> <pd> <od> [n]  private photo variants, output to scratch <od>
#   file <nap> <mp> <f> <tag>   one extra synthetic file (C:\TEST\)
#   on                   every sample, memprot=on (default), dumps at 30/45 s
#   strict <name...>     the named samples with memprot=strict
#   regr                 HEAD kernel, memprot on: APITEST, QED (#Z), TeraDesk (#Z)
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
K="$S/kernel8.img"; BASE="$S/base.img"; C="$S/cards"; mkdir -p "$C"
OFF=$((63 * 512))
ZB="$WS/measurements/measurement-2026-10-06-zview-m4/bin"
SAMPLES="T24.BMP T8.BMP T24.TGA TRLE.TGA TRGB.PNG TPAL.PNG T.GIF T.JPG T.PI1 T.PC1 T.NEO T.GOD T.RAW T4.IMG T1.IMG"

card() { # card <out> <autorun> [zvarg]
  local out=$1 auto=$2 arg=${3:-}
  cp "$BASE" "$out"
  sed "s|^#Z .*|#Z 01 ${auto//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mcopy -o -i "$out@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
  mcopy -o -i "$out@@$OFF" "$ZB/ZVLAUNCH.NAP" ::ZVLAUNCH.NAP
  if [ -n "$arg" ]; then printf '%s\r\n' "$arg" > "$S/ZVARG.TXT"; mcopy -o -i "$out@@$OFF" "$S/ZVARG.TXT" ::ZVARG.TXT; fi
  mcopy -o -i "$out@@$OFF" "$WS/sdk/apps/apitest/APITEST.NCP" ::APITEST.NCP
}
r4() { bash "$HERE/run-rpi4.sh" "$@"; }
tag() { echo "$1" | tr 'A-Z.' 'a-z-'; }

part=${1:?part}; shift
case "$part" in
plprobe)
  # plplprobe: PLPROBE.NAP (plprobe.c) in place of ZVIEW.NAP -- the
  # plugins_init() steps printed to serial
  card "$C/plprobe.img" 'C:\ZVLAUNCH.NAP' 'C:\ZVIEW\SAMPLES\T24.BMP'
  mcopy -o -i "$C/plprobe.img@@$OFF" "$ZB/PLPROBE.NAP" ::ZVIEW/ZVIEW.NAP
  APPEND=serial=on r4 "$K" "$C/plprobe.img" plprobe 20:t20.png
  ;;
alt)
  # alt <nap> <sample> <tag>: a diagnostic build (scratch, instrumented
  # copy of one zView file -- see RUNINFO) in place of C:\ZVIEW\ZVIEW.NAP
  nap=${1:?nap}; n=${2:?sample}; t=${3:?tag}
  card "$C/alt.img" 'C:\ZVLAUNCH.NAP' "C:\\ZVIEW\\SAMPLES\\$n"
  mcopy -o -i "$C/alt.img@@$OFF" "$nap" ::ZVIEW/ZVIEW.NAP
  APPEND="${APPEND:-serial=on}" r4 "$K" "$C/alt.img" "alt-$t" 30:t30.png
  ;;
diag)
  # diag <nap> <on|strict|off> [samples]: every sample with a diagnostic
  # ZVIEW.NAP (finding K1, see RUNINFO), memprot as named
  nap=${1:?nap}; mp=${2:?memprot}; shift 2
  for n in ${@:-$SAMPLES}; do
    card "$C/diag.img" 'C:\ZVLAUNCH.NAP' "C:\\ZVIEW\\SAMPLES\\$n"
    mcopy -o -i "$C/diag.img@@$OFF" "$nap" ::ZVIEW/ZVIEW.NAP
    A="serial=on"; [ "$mp" != on ] && A="serial=on memprot=$mp"
    APPEND="$A" r4 "$K" "$C/diag.img" "diag-$mp-$(tag "$n")" 30:t30.png
  done
  ;;
photo)
  # photo <nap> <mp> <photo dir> <out dir> [names]: the PRIVATE photo
  # variants (gen-photo-variants.py) from <photo dir>/IMAGES, copied to
  # C:\PHOTO\ of a scratch card; screendumps and serial logs go to <out dir>
  # (scratch), never into this run directory.
  nap=${1:?nap}; mp=${2:?memprot}; PD=${3:?photo dir}; PO=${4:?out dir}; shift 4
  for n in ${@:-P.JPG P.PNG P.BMP P.TGA P.GIF P.GOD P.RAW P8.IMG P.PI1 P.NEO}; do
    card "$C/photo.img" 'C:\ZVLAUNCH.NAP' "C:\\PHOTO\\$n"
    mmd -i "$C/photo.img@@$OFF" ::PHOTO 2>/dev/null || true
    mcopy -o -i "$C/photo.img@@$OFF" "$PD/IMAGES/$n" "::PHOTO/$n"
    mcopy -o -i "$C/photo.img@@$OFF" "$nap" ::ZVIEW/ZVIEW.NAP
    A="serial=on"; [ "$mp" != on ] && A="serial=on memprot=$mp"
    OUTDIR="$PO" SERDIR="$PO" APPEND="$A" r4 "$K" "$C/photo.img" "photo-$mp-$(tag "$n")" 30:t30.png
  done
  rm -f "$C/photo.img"
  ;;
file)
  # file <nap> <mp> <host file> <tag>: one extra (synthetic) file as
  # C:\TEST\<name>, opened by <nap> as ZVIEW.NAP
  nap=${1:?nap}; mp=${2:?memprot}; f=${3:?file}; t=${4:?tag}; b=$(basename "$f")
  card "$C/file.img" 'C:\ZVLAUNCH.NAP' "C:\\TEST\\$b"
  mmd -i "$C/file.img@@$OFF" ::TEST
  mcopy -o -i "$C/file.img@@$OFF" "$f" "::TEST/$b"
  mcopy -o -i "$C/file.img@@$OFF" "$nap" ::ZVIEW/ZVIEW.NAP
  A="serial=on"; [ "$mp" != on ] && A="serial=on memprot=$mp"
  APPEND="$A" r4 "$K" "$C/file.img" "file-$mp-$t" 30:t30.png
  ;;
probe)
  n=${1:?sample}; card "$C/probe.img" 'C:\ZVLAUNCH.NAP' "C:\\ZVIEW\\SAMPLES\\$n"
  APPEND=serial=on r4 "$K" "$C/probe.img" "probe-$(tag "$n")" 15:t15.png 25:t25.png 35:t35.png 50:t50.png
  ;;
on)
  for n in ${@:-$SAMPLES}; do
    card "$C/on.img" 'C:\ZVLAUNCH.NAP' "C:\\ZVIEW\\SAMPLES\\$n"
    APPEND=serial=on r4 "$K" "$C/on.img" "on-$(tag "$n")" 30:t30.png 45:t45.png
  done
  ;;
strict)
  for n in "$@"; do
    card "$C/strict.img" 'C:\ZVLAUNCH.NAP' "C:\\ZVIEW\\SAMPLES\\$n"
    APPEND="serial=on memprot=strict" r4 "$K" "$C/strict.img" "strict-$(tag "$n")" 30:t30.png 45:t45.png
  done
  ;;
regr)
  card "$C/apitest.img" 'C:\APITEST.NCP'
  card "$C/qed.img" 'C:\Apps\QED\QED.NAP'
  card "$C/teradesk.img" 'C:\Apps\TERADESK\DESKTOP.NAP'
  APPEND=serial=on r4 "$K" "$C/apitest.img"  regr-on-apitest  25:t25.png 40:t40.png
  APPEND=serial=on r4 "$K" "$C/qed.img"      regr-on-qed      30:t30.png
  APPEND=serial=on r4 "$K" "$C/teradesk.img" regr-on-teradesk 35:t35.png
  ;;
*) echo "part? $part"; exit 2;;
esac
