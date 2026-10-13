#!/usr/bin/env bash
# vreg-kernels.sh <scratch-root> -- qemu-virt scratch kernels with virtreg-instrumentation.py (probes in the RAM
# registry, launched at the AUTO position), from scratch copies of HEAD (make-scratch.sh):
#   kernel-virt-vreg-ac.img      STDOUTDB, STDERRP (new crt0), FS11A, STDERRA, APITEST.NCP  (bin/)
#   kernel-virt-vreg-old-ac.img  STDERRP built with the OLD crt0 (bin/oldcrt0/), STDERRA is the new-crt0 ARGV kid
set -euo pipefail
R="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; BIN="$HERE/../bin"
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
mkdir -p "$R/vprogs"; cp "$BIN/STDOUTDB.NAP" "$BIN/STDERRP.NAP" "$BIN/FS11A.NAP" "$BIN/STDERRA.NAP" "$BIN/APITEST.NCP" "$R/vprogs/" 2>/dev/null || cp "$HERE/../../../sdk/apps/apitest/APITEST.NCP" "$R/vprogs/"
mkdir -p "$R/vprogs-old"; cp "$BIN/oldcrt0/STDERRP.NAP" "$R/vprogs-old/"; cp "$BIN/STDERRA.NAP" "$R/vprogs-old/"
b() { # b <dirname> <progs-dir> <names> <out>
  sh "$HERE/make-scratch.sh" "$R/$1"
  VREG_NAMES="$3" python3 "$HERE/virtreg-instrumentation.py" "$R/$1" "$2" > /dev/null
  (cd "$R/$1" && cargo xtask build --board qemu-virt --release > "$R/$1.log" 2>&1) || { tail -20 "$R/$1.log"; exit 1; }
  "$OC" -O binary "$R/$1/target/aarch64-unknown-none-softfloat/release/kernel" "$BIN/$4"; echo "built $4"
}
b vreg "$R/vprogs" "STDOUTDB.NAP,STDERRP.NAP,FS11A.NAP,STDERRA.NAP,APITEST.NCP" kernel-virt-vreg-ac.img
b vregold "$R/vprogs-old" "STDERRP.NAP,STDERRA.NAP" kernel-virt-vreg-old-ac.img
