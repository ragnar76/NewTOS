#!/usr/bin/env bash
# series-u6a-prod.sh -- second half of the DESK43 U6a QEMU raspi4b series,
# after checks.sh had built the product kernel (bin/kernel8-u6a.img,
# unpatched) and the host was idle again. One run at a time.
#   rerun of inj-on-zpanic (the first one stalled 670 s under host load
#   while checks.sh was compiling, see RUNINFO), then on the product kernel:
#   zpanic in on/off/strict (needs no key) and z in on (no key: the window,
#   then the driver's timeout -- the program waits for its key, by design).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="$HERE/../bin"; C=/tmp/m5j/u6a
APPEND="serial=on" "$HERE/run-u6a.sh" "$B/kernel8-inj.img" "$C/card-zpanic.img" "inj-on-zpanic-rerun" --timeout 240
for M in on off strict; do
  case $M in on) P="";; off) P="memprot=off";; strict) P="memprot=strict";; esac
  APPEND="$(echo "serial=on $P" | xargs)" "$HERE/run-u6a.sh" "$B/kernel8-u6a.img" "$C/card-zpanic.img" "prod-$M-zpanic" --timeout 240
done
APPEND="serial=on" "$HERE/run-u6a.sh" "$B/kernel8-u6a.img" "$C/card-z.img" "prod-on-z" --timeout 45
