#!/usr/bin/env bash
# zp-runs.sh <scratch root> <kernel> <tag> [photos] [drive options...]
# ZVPROF (measurement-2026-10-07-zview-perf): one QEMU raspi4b run of zView
# (#Z C:\ZVIEW\ZVIEW.NAP) on a cp -c clone of <S>/base.img
# (tools/make-sd-image.sh <S>/base.img 64). "photos" adds the 8 generated
# photo JPEGs (<S>/photos/X0n.JPG) to C:\ZVIEW\SAMPLES. The clone is deleted.
set -uo pipefail
S="${1:?scratch root}"; K="${2:?kernel}"; T="${3:?tag}"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OFF=$((63 * 512)); C="$S/cards"; mkdir -p "$C"
card="$C/zp.img"; rm -f "$card"; cp -c "$S/base.img" "$card"
sed "s|^#Z .*|#Z 01 C:\\\\ZVIEW\\\\ZVIEW.NAP@|" "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
mcopy -o -i "$card@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
if [ "${1:-}" = photos ]; then
  shift
  for f in "$S"/photos/*.JPG; do mcopy -o -i "$card@@$OFF" "$f" "::ZVIEW/SAMPLES/$(basename "$f")"; done
fi
export SOCKROOT="$S/sock"; mkdir -p "$SOCKROOT"
bash "$HERE/run-rpi4-drive-m3.sh" "$K" "$card" "$T" --end '\[m3\] MARK END' --end-delay 4 --timeout 300 "$@"
rm -f "$card"
