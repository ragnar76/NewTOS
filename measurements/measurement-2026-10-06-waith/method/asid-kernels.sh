#!/usr/bin/env bash
# asid-kernels.sh <label> <scratch-root> -- the ARCH03-4 probe kernels, from
# scratch copies of the shared tree (make-scratch.sh; never the shared tree):
#   kernel8-asidk-<L>.img      board-rpi4: measurement-2026-10-05-arch03-0c
#                              apply-instrumentation.py (IKBD injector answers
#                              the crash prompt of ASIDSTR part C) +
#                              asid-instrumentation.py (switch-cost counters)
#   kernel-virt-asidk-<L>.img  board-qemu-virt: asid-instrumentation.py
#                              --virt-launch (counters + ASIDSTR from the RAM
#                              registry at the AUTO position)
#   kernel8-neg-notlbi/-noinval-<L>.img  negative controls (NEG=0 skips)
set -euo pipefail
L="${1:?label}"; R="${2:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/../.." && pwd)"
BIN="$HERE/../bin"
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
mk() { sh "$HERE/make-scratch.sh" "$R/$1"; }
bld() { # bld <dir> <board> <out>
  (cd "$R/$1" && cargo xtask build --board "$2" --release > "$R/$1-$2.log" 2>&1) || { tail -20 "$R/$1-$2.log"; exit 1; }
  if [ "$2" = rpi4 ]; then cp "$R/$1/kernel8.img" "$BIN/$3"; else
    "$OC" -O binary "$R/$1/target/aarch64-unknown-none-softfloat/release/kernel" "$BIN/$3"; fi
  echo "built $3"
}
mk asidk; python3 "$M/measurement-2026-10-05-arch03-0c/method/apply-instrumentation.py" "$R/asidk" "$BIN" > /dev/null
python3 "$HERE/asid-instrumentation.py" "$R/asidk" "$BIN" > /dev/null
bld asidk rpi4 "kernel8-asidk-$L.img"
mk asidkv; python3 "$HERE/asid-instrumentation.py" "$R/asidkv" "$BIN" --virt-launch > /dev/null
bld asidkv qemu-virt "kernel-virt-asidk-$L.img"
# negative controls (deliberately broken, see asid-instrumentation.py)
if [ "${NEG:-1}" = 1 ]; then
  mk negtlbi; python3 "$M/measurement-2026-10-05-arch03-0c/method/apply-instrumentation.py" "$R/negtlbi" "$BIN" > /dev/null
  python3 "$HERE/asid-instrumentation.py" "$R/negtlbi" "$BIN" --neg-notlbi > /dev/null
  bld negtlbi rpi4 "kernel8-neg-notlbi-$L.img"
  mk neginv; python3 "$M/measurement-2026-10-05-arch03-0c/method/apply-instrumentation.py" "$R/neginv" "$BIN" > /dev/null
  python3 "$HERE/asid-instrumentation.py" "$R/neginv" "$BIN" --neg-noinval > /dev/null
  bld neginv rpi4 "kernel8-neg-noinval-$L.img"
fi
