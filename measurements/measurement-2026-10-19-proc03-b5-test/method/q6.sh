#!/usr/bin/env bash
# q6.sh -- A112 f2 repeated (jitter check of the [sched] trace line count), head/wt x3, then the qemu-virt smoke.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; B="$S/bin"
for i in 1 2 3; do for side in head wt; do
  CASES=f2 bash "$HERE/a112/run-all.sh" "$B/kernel8-$side-plain.img" "$side-rep$i" "$S/a112" >/dev/null 2>&1
done; done
bash "$HERE/virt-smoke.sh" "$B/kernel-virt-wt.img" "$S"
echo Q6-DONE
