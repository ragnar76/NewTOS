#!/usr/bin/env bash
# vreg-tty-kernel.sh <scratch-root> -- qemu-virt scratch kernel (RAM registry, no C:) with TTYA.NAP + TTYPROBE.NAP
# (TTYPROBE only as TTYA's registered child path; launched second, blocks on its T4 keys, run is ended before).
set -euo pipefail
R="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; BIN="$HERE/../bin"
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
mkdir -p "$R/vprogs"; cp "$BIN/TTYA.NAP" "$BIN/TTYPROBE.NAP" "$R/vprogs/"
sh "$HERE/make-scratch.sh" "$R/vtty"
VREG_NAMES="TTYA.NAP,TTYPROBE.NAP" python3 "$HERE/virtreg-instrumentation.py" "$R/vtty" "$R/vprogs" > /dev/null
(cd "$R/vtty" && cargo xtask build --board qemu-virt --release > "$R/vtty.log" 2>&1) || { tail -20 "$R/vtty.log"; exit 1; }
"$OC" -O binary "$R/vtty/target/aarch64-unknown-none-softfloat/release/kernel" "$BIN/kernel-virt-vreg-tty.img"; echo "built kernel-virt-vreg-tty.img"
