#!/usr/bin/env bash
# ZVIEW04 copy: the zview part keeps the run card until AESTRACE.BIN is copied out.
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
  KEEP=1 bash "$HERE/run-rpi4-drive-m3.sh" "$k" "$C/zv.img" "$t" --end '\[m3\] MARK END' --end-delay 2 --timeout 240
  # ZVIEW04: keep C:\AESTRACE.BIN (aes-trace kernels) from the run copy, then delete it
  mcopy -n -i "$C/run-$t.img@@$OFF" ::AESTRACE.BIN "$S/aestrace-$t.bin" 2>/dev/null && echo "trace: $S/aestrace-$t.bin"
  shasum -a 256 "$C/run-$t.img" | sed 's|  .*|  run-'"$t"'.img (deleted)|'
  rm -f "$C/zv.img" "$C/run-$t.img"
  ;;
regr)
  k=${1:?kernel}; t=${2:?tag}; auto=${3:?autorun}; shift 3
  card "$C/rg.img" "$auto"
  bash "$HERE/run-rpi4-drive-m3.sh" "$k" "$C/rg.img" "$t" "$@"
  rm -f "$C/rg.img"
  ;;
*) echo "part? $part"; exit 2;;
esac
