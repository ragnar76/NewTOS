#!/usr/bin/env bash
# rerun-uacc.sh <label> <kernel-label> <scratch-root> -- after a probe-only
# change (sdk/examples/uakid.c / uacc3a.c, no product change): rebuild the
# probes, rebuild only the virt crash kernel that embeds them, and repeat the
# two UACC3A runs (raspi4b crash kernel <kernel-label> with a fresh card; virt
# crash kernel <label>). Serial/screendumps under <label>-*.
set -euo pipefail
L="${1:?label}"; K="${2:?kernel label}"; R="${3:?scratch root}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/../.." && pwd)"
BIN="$HERE/../bin"
OC="$(rustc --print sysroot)/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy"
bash "$HERE/build-probes.sh" "$BIN" > "$HERE/../checks/$L-probes.txt" 2>&1
sh "$HERE/make-scratch.sh" "$R/scr/crash"
python3 "$M/measurement-2026-10-05-arch03-0c/method/apply-instrumentation.py" "$R/scr/crash" "$BIN" > /dev/null
python3 "$HERE/uacc-instrumentation.py" "$R/scr/crash" "$BIN" > /dev/null
(cd "$R/scr/crash" && cargo xtask build --board qemu-virt --release > "$R/scr/crash-qemu-virt-$L.log" 2>&1)
"$OC" -O binary "$R/scr/crash/target/aarch64-unknown-none-softfloat/release/kernel" "$BIN/kernel-virt-crash-$L.img"
mkdir -p "$R/cards"
bash "$HERE/seed-card.sh" "$R/cards/uacc.img" 'C:\UACC3A.NAP' > /dev/null
bash "$HERE/run-rpi4-drive.sh" "$BIN/kernel8-crash-$K.img" "$R/cards/uacc.img" "$L-rpi4-uacc3a" --end 'UACC3A: (PASS|FAIL)' --end-delay 8 --timeout 600
bash "$HERE/run-virt.sh" "$BIN/kernel-virt-crash-$L.img" "$L-virt-crash" --answer --timeout 400
