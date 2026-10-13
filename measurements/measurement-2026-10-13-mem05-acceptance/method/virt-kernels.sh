#!/usr/bin/env bash
# virt-kernels.sh <label> -- scratch qemu-virt kernels with MEM05 probes from the RAM registry (mdv-instrumentation.py)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; M="$HERE/.."; L="$1"; R=/tmp/m5j/acc/sk
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
for spec in "mdvs MDSTEP.NAP" "mdvp MDPAR.NAP MDKID.NAP" "mdvb MDBUD.NAP" "mdvh HELLOMEM.NAP" "mdvq MDPROBE.NAP"; do
  set -- $spec; d=$1; shift
  bash "$HERE/make-scratch.sh" $R/$d && python3 "$HERE/mdv-instrumentation.py" $R/$d "$M/bin" "$@" && \
  (cd $R/$d && cargo xtask build --board qemu-virt --release > $R/$d-virt.log 2>&1 && $OC -O binary target/aarch64-unknown-none-softfloat/release/kernel "$M/bin/kernel-virt-$d-$L.img" && echo built $d) || tail -20 $R/$d-virt.log
done
