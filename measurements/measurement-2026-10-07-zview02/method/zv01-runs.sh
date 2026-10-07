#!/usr/bin/env bash
# zv01-runs.sh <scratch root> <part> ... -- the QEMU raspi4b runs of TASK/ZVIEW01
# (serial=on, memprot default on unless APPEND says otherwise). One QEMU at a time.
# Card: <S>/base.img = tools/make-sd-image.sh <S>/base.img 64 (QEMU image only;
# installs C:\ZVIEW\ from sdk/zview/build/dist). Per run a private `cp -c` clone
# gets C:\NEWDESK.INF (newdesk-template.inf with "#Z 01 <autorun>@"); for the
# zview part optionally <nap> replaces C:\ZVIEW\ZVIEW.NAP. The clone is deleted
# after the run (run-rpi4-drive-m3.sh copies it once more and deletes that copy).
# Parts:
#   zview <kernel> <tag> [nap]   #Z C:\ZVIEW\ZVIEW.NAP; the kernel carries the injector
#                                (kinj-zv-browser.img or kinj-zv-samples.img)
#   regr  <kernel> <tag> <auto>  #Z <auto> (TeraDesk, QED, APITEST ...)
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$S/base.img"; C="$S/cards"; mkdir -p "$C"
OFF=$((63 * 512))
card() { # card <out> <autorun>
  local out=$1 auto=$2
  rm -f "$out"; cp -c "$BASE" "$out"
  sed "s|^#Z .*|#Z 01 ${auto//\\/\\\\}@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
  mcopy -o -i "$out@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
}
part=${1:?part}; shift
case "$part" in
zview)
  k=${1:?kernel}; t=${2:?tag}; nap=${3:-}
  card "$C/zv.img" 'C:\ZVIEW\ZVIEW.NAP'
  [ -n "$nap" ] && mcopy -o -i "$C/zv.img@@$OFF" "$nap" ::ZVIEW/ZVIEW.NAP
  bash "$HERE/run-rpi4-drive-m3.sh" "$k" "$C/zv.img" "$t" --end '\[m3\] MARK END' --end-delay 2 --timeout 240
  rm -f "$C/zv.img"
  ;;
regr)
  k=${1:?kernel}; t=${2:?tag}; auto=${3:?autorun}; shift 3
  card "$C/rg.img" "$auto"
  bash "$HERE/run-rpi4-drive-m3.sh" "$k" "$C/rg.img" "$t" "$@"
  rm -f "$C/rg.img"
  ;;
*) echo "part? $part"; exit 2;;
esac
