#!/usr/bin/env bash
# series-hellogem.sh -- DESK43 U6b: "HELLOGEM still runs" with the merged newtos-gem bindings.
# HELLOGEM.NAP = bin/HELLOGEM-rpi4.NAP (cargo xtask build-programs --board rpi4 of the U6b tree), seeded by
# seed-u6a.sh onto fresh cards /tmp/m5j/u6b/card-z.img (#Z 01 C:\HELLOGEM.NAP@) and card-f1.img (installed on F1,
# started by the desktop = the desktop's launch path through the moved AES bindings). Kernel: bin/kernel8-inj.img
# (build-kernel-inj.sh: U6b tree + inj-u6a.py key injector; QEMU raspi4b has no USB keyboard).
# z in memprot on/off/strict, f1 in on. One QEMU at a time.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; C=/tmp/m5j/u6b
for M in on off strict; do
  case $M in on) P="";; off) P="memprot=off";; strict) P="memprot=strict";; esac
  APPEND="$(echo "serial=on $P" | xargs)" "$HERE/run-u6a.sh" "$B/kernel8-inj.img" "$C/card-z.img" "hg-inj-$M-z" --timeout 240
done
APPEND="serial=on" "$HERE/run-u6a.sh" "$B/kernel8-inj.img" "$C/card-f1.img" "hg-inj-on-f1" --timeout 240
