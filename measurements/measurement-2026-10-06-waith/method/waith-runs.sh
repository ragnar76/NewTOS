#!/usr/bin/env bash
# waith-runs.sh -- TASK/PORT-WAITH: what this run did, in order (method/ is
# measurement-2026-10-06-arch03-4/method copied; changed: tests/vforkt.c,
# seed-card.sh tolerates bin/ without .NTP; new: this file).
#   $1 = scratch root (session scratchpad), never the repo or shared /tmp.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN="$(cd "$HERE/.." && pwd)"; WS="$(cd "$RUN/../.." && pwd)"
S="${1:?scratch root}"
# 1. red: the host harness against mintlib's pristine header
mkdir -p "$S/pristine/sys"
tar xOf "$WS/sdk/libc/mintlib-0.60.1.tar.xz" mintlib-0.60.1/include/sys/wait.h > "$S/pristine/sys/wait.h"
WAITH_HEADER_DIR="$S/pristine" "$WS/sdk/libc/tests/waith-host.sh" > "$RUN/checks/red-waith-host-pristine-header.txt" 2>&1
# 2. libc rebuild (applies the overlay), green host harness
"$WS/sdk/libc/build-libc.sh" > "$RUN/checks/build-libc.txt" 2>&1
"$WS/sdk/libc/tests/waith-host.sh" > "$RUN/checks/green-waith-host.txt" 2>&1
# 3. probes (all 25 rebuilt; only VFORKT.NAP differs from arch03-4's bin)
bash "$HERE/build-s2-tests.sh" "$S/probes" > "$RUN/checks/build-s2-tests.txt" 2>&1
cp "$S/probes/VFORKT.NAP" "$RUN/bin/VFORKT.NAP"
# 4. static checks + product kernels (bin/kernel8-waith.img, kernel-virt-waith.img)
bash "$HERE/checks.sh" waith | tee "$RUN/checks/checks-summary-waith.txt"
# 5. QEMU raspi4b, memory protection on (default), serial=on
bash "$HERE/seed-card.sh" "$S/waith-cards/vforkt.img" 'C:\VFORKT.NAP'
bash "$HERE/seed-card.sh" "$S/waith-cards/s2all.img" 'C:\S2ALL.NAP'
APPEND="serial=on" bash "$HERE/run-rpi4-drive.sh" "$RUN/bin/kernel8-waith.img" "$S/waith-cards/vforkt.img" \
  waith-on-rpi4-vforkt-alone --end 'VFORK: (PASS|FAIL)' --end-delay 3 --timeout 240
APPEND="serial=on" bash "$HERE/run-rpi4-drive.sh" "$RUN/bin/kernel8-waith.img" "$S/waith-cards/s2all.img" \
  waith-on-rpi4-s2all --end 'S2ALL: (PASS|FAIL)' --end-delay 2 --timeout 400
