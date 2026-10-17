#!/bin/bash
# usage: run-idprobe.sh <kernel8.img> <tag red|green> <scratch-dir> [secs]
# Builds the four idprobe roles with sdk/build.sh into <scratch>/bin (ACCX.NAC,
# ACCY.NAC, MAIN.NAP, SECOND.NAP), one FAT16 image (mkimg.sh, volume EVPRB,
# NEWDESK.INF '#Z 01 C:\MAIN.NAP@'), runs it once in QEMU raspi4b
# (runts.sh: APFS clone, -append serial=on, host timestamps) and writes
# ../serial/idprobe-<tag>.log plus ../serial/probes-<tag>.sha256.
set -e
K=$1; TAG=$2; W=$3; SECS=${4:-45}
HERE="$(cd "$(dirname "$0")" && pwd)"; SDK="$HERE/../../../sdk"; SER="$HERE/../serial"
mkdir -p "$W/bin" "$W/img" "$SER"
b() { bash "$SDK/build.sh" "$HERE/r_$1.c" "$W/bin/$2" >/dev/null; }
b ACCX ACCX.NAC; b ACCY ACCY.NAC; b MAIN MAIN.NAP; b SECOND SECOND.NAP
shasum -a 256 "$W"/bin/* | sed "s|$W/||" > "$SER/probes-$TAG.sha256"
rm -f "$W/img/idprobe.img"
bash "$HERE/mkimg.sh" "$W/img/idprobe.img" "$W/bin/ACCX.NAC" "$W/bin/ACCY.NAC" "$W/bin/MAIN.NAP" "$W/bin/SECOND.NAP" >/dev/null
bash "$HERE/runts.sh" "$K" "$W/img/idprobe.img" "$SER/idprobe-$TAG.log" "$SECS"
rm -f "$W/img/idprobe.img"
