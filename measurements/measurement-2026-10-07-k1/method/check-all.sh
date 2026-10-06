#!/usr/bin/env bash
# check-all.sh <shot dir> <run prefix> <expected dir> [samples] -- runs
# check-shot.py on <shot dir>/<prefix>-<sample tag>-t30.png for every sample;
# the mouse sprite (QEMU boot position, footprint measured on the plain
# desktop of alt-vqprobe-t30.png: x 319-329, y 239-254) is masked. T.JPG and
# P.JPG use <expected dir>/<name>.raw.rgb (jpgref.c) with --raw.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
D=$1; P=$2; E=$3; shift 3
SAMPLES="${*:-T24.BMP T8.BMP T24.TGA TRLE.TGA TRGB.PNG TPAL.PNG T.GIF T.JPG T.PI1 T.PC1 T.NEO T.GOD T.RAW T4.IMG T1.IMG}"
pass=0; fail=0
for n in $SAMPLES; do
  t=$(echo "$n" | tr 'A-Z.' 'a-z-')
  if [ -f "$E/$n.raw.rgb" ]; then exp="$E/$n.raw.rgb"; raw=--raw; else exp="$E/$n.rgb"; raw=; fi
  if python3 "$HERE/check-shot.py" "$D/$P-$t-t30.png" "$exp" $raw --mask 319,239,330,255; then
    pass=$((pass+1)); else fail=$((fail+1)); fi
done
echo "TOTAL $P: $pass PASS, $fail FAIL"
