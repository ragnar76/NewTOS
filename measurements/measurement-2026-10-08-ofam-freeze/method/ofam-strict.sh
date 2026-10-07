#!/usr/bin/env bash
# OFAM freeze: memprot=strict, once: EmuDESK (evb-desk), zView browser (zv-browser), APITEST.
set -uo pipefail
S="${1:?scratch}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C="$S/cards"; export SOCKROOT="$S"
A="serial=on memprot=strict"
APPEND="$A" bash "$HERE/run-rpi4-drive-m3.sh" "$S/kinj-evb-desk.img" "$C/desk.img" desk-strict --mark-delay 0.3 --end '\[m3\] MARK END' --end-delay 1 --timeout 150
APPEND="$A" bash "$HERE/zv01-runs.sh" "$S" zview "$S/kinj-zv-browser.img" zv-browser-strict
APPEND="$A" bash "$HERE/run-rpi4.sh" "$S/kernel8-evb.img" "$C/apitest.img" api-strict 25:t25.png 40:t40.png
