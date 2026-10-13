#!/usr/bin/env bash
# scratch-kernels-part2.sh (LIBC-TTY-ACCEPT: the keys/tdkeys/keys9 part only, rerun after an rsync error of the first pass; copy of scratch-kernels.sh)
# scratch-kernels.sh <label> <scratch-root> -- build the instrumented kernels
# (stage 3a: + uacc-instrumentation.py on the crash kernel: UACC3A on virt)
# of a sub-step from scratch copies of the shared tree (make-scratch.sh; the
# instrumentation is never applied to the shared tree):
#   kernel8-crash-<L>, kernel-virt-crash-<L>: measurement-2026-10-05-arch03-0c
#       apply-instrumentation.py (IKBD injector answers the crash prompt on
#       raspi4b; CRCHILD + CRASHTST from the RAM registry on virt)
#   kernel8-keys-<L>: measurement-2026-10-04-keys02 apply-instrumentation.py +
#       emucon-schedule.py (KBDVEC + EmuCON session)
#   kernel-virt-st-<L>: wire-selftests.py (the ten hand-built-ELF selftests)
# stage 3c: + e10-instrumentation.py on the crash kernel (E10 probe on virt)
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
mk keys; python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/keys" > /dev/null
python3 "$HERE/emucon-schedule.py" "$R/keys" > /dev/null
bld keys rpi4 "kernel8-keys-$L.img"
# E9/post-E9: the TeraDesk->QED (F1) and EmuCON-five-programs injector kernels
mk tdkeys; python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/tdkeys" > /dev/null
python3 "$HERE/tdqed-keys.py" "$R/tdkeys" > /dev/null
bld tdkeys rpi4 "kernel8-tdkeys-$L.img"
mk keys9; python3 "$HERE/inj-keys02-apply-instrumentation.py" "$R/keys9" > /dev/null
python3 "$HERE/emucon-schedule-e9.py" "$R/keys9" > /dev/null
bld keys9 rpi4 "kernel8-keys9-$L.img"
