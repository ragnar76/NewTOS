#!/usr/bin/env bash
# regr-input01.sh <scratch root> <tag prefix> [extra boot args]
# INPUT01 click/menu regression = the ofam-freeze group A + DOOM
# (measurement-2026-10-08-ofam-freeze/method/ofam-groupA.sh + ofam-doom.sh),
# serially, one QEMU at a time, on the INPUT01 kernels <S>/new-kinj-*.img and
# <S>/new-kernel8-evb.img (make-kernels-input01.sh). Tags <prefix>-<run>;
# extra boot args (e.g. "input=aes") go into every run's -append.
#   evb-runs.sh: accept (LDGDEMO on/strict), desk, tera, qed, desk3a, api
#   zv01-runs.sh zview: zv-dclick, zv-browser, zv-quit   (card BASE=<S>/base64.img)
#   ofam-doom.sh: DOOM start / play / quit              (card BASE=<S>/base64.img)
set -uo pipefail
S="${1:?scratch root}"; L="${2:?tag prefix}"; X="${3:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export SOCKROOT="$S" KP=new- TP="$L-" XA="$X"
bash "$HERE/evb-runs.sh" "$S" accept desk tera qed desk3a api
for x in zv-dclick zv-browser zv-quit; do
  APPEND="serial=on${X:+ $X}" BASE="$S/base64.img" bash "$HERE/zv01-runs.sh" "$S" zview "$S/new-kinj-$x.img" "$L-$x"
done
BASE="$S/base64.img" bash "$HERE/ofam-doom.sh" "$S"
echo "=== regression $L done"
