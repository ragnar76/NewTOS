#!/usr/bin/env bash
# virt-mdv.sh <label> [modes...] -- MEM05 probes on qemu-virt from the RAM registry (scratch kernels kernel-virt-mdv?-<label>.img,
# virt-kernels.sh): MDSTEP (mdlimit=128), MDPAR+MDKID (mdlimit=64), MDPROBE (default), MDBUD (mdlimit=0), HELLOMEM, plus the
# mdlimit= boot-argument lines on the product kernel. cwd = /tmp/m5j/acc.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; B="$HERE/../bin"; L="${1:?}"; shift
for m in ${*:-on off strict on-asid}; do
  M="$m"; . "$HERE/mode.sh"
  run() { # run <kernel> <tag> <extra append> [drive opts]
    local k="$1" t="$2" x="$3"; shift 3
    APPEND="$(echo "$AV $x" | xargs)" bash "$HERE/run-virt.sh" "$B/$k" "$L-$m-virt-$t" "$@"
  }
  run "kernel-virt-mdvh-$L.img" hellomem "" --end '\[mdv\] back in the boot path' --end-delay 3 --timeout 300 --stamp
  run "kernel-virt-mdvs-$L.img" mdstep "mdlimit=128" --end '\[mdv\] back in the boot path' --end-delay 3 --timeout 1500 --stamp
  run "kernel-virt-mdvq-$L.img" mdprobe "" --end '\[mdv\] back in the boot path' --end-delay 3 --timeout 1500 --stamp
  run "kernel-virt-mdvp-$L.img" mdpar "mdlimit=64" --end '\[mdv\] back in the boot path' --end-delay 3 --timeout 600 --stamp
  run "kernel-virt-mdvb-$L.img" mdbud "mdlimit=0" --end '\[desk\] desktop up' --end-delay 8 --timeout 3000 --stamp
  run "kernel-virt-$L.img" bootarg-mdl64 "mdlimit=64" --timeout 120
  run "kernel-virt-$L.img" bootarg-mdl0 "mdlimit=0" --timeout 120
  run "kernel-virt-$L.img" bootarg-mdlbad "mdlimit=12x" --timeout 120
done
