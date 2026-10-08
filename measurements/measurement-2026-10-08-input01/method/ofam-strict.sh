#!/usr/bin/env bash
# INPUT01 copy: kernels with prefix KP, tags with prefix TP, extra boot args XA.
# OFAM freeze: memprot=strict, once: EmuDESK (evb-desk), zView browser (zv-browser), APITEST.
set -uo pipefail
S="${1:?scratch}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C="$S/cards"; export SOCKROOT="$S"
A="serial=on${XA:+ $XA} memprot=strict"
APPEND="$A" bash "$HERE/run-rpi4-drive-m3.sh" "$S/${KP:-}kinj-evb-desk.img" "$C/desk.img" ${TP:-}desk-strict --mark-delay 0.3 --end '\[m3\] MARK END' --end-delay 1 --timeout 150
APPEND="$A" bash "$HERE/zv01-runs.sh" "$S" zview "$S/${KP:-}kinj-zv-browser.img" ${TP:-}zv-browser-strict
APPEND="$A" bash "$HERE/run-rpi4.sh" "$S/${KP:-}kernel8-evb.img" "$C/apitest.img" ${TP:-}api-strict 25:t25.png 40:t40.png
