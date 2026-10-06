#!/usr/bin/env bash
# libtest-runs.sh <scratch root> [mode...] -- the QEMU raspi4b runs of
# TASK/LDG-M2: \AUTO\LIBTEST.NCP on the minimal card, kernel
# ../bin/kernel8-ldgm2.img (HEAD 6a925cc, unchanged -- no kernel source
# touched by this task), -append "serial=on" (+ memprot=<mode> unless "on").
# Ends when the verdict line appears (drive.py --end), then 2 s + screendump.
set -uo pipefail
S="${1:?scratch root}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K="$HERE/../bin/kernel8-ldgm2.img"; C="$S/ldgm2-cards"
mkdir -p "$C"
[ -f "$C/libtest.img" ] || bash "$HERE/seed-card-libtest.sh" "$C/libtest.img"
END='libtest: (ALL PASS|FAIL \()'
for M in "$@"; do
  A="serial=on"; [ "$M" != on ] && A="serial=on memprot=$M"
  APPEND="$A" bash "$HERE/run-rpi4-drive.sh" "$K" "$C/libtest.img" "ldgm2-$M-rpi4-libtest" \
    --end "$END" --end-delay 2 --timeout 300
done
