#!/usr/bin/env bash
# run-virt.sh <kernel.img(flat)> <outdir> <tag> <SPEC...>   (SPEC = SECONDS:PNG)
# The qemu-virt half of run-rpi4.sh.  Same machine line as `xtask run --board
# qemu-virt` (tools/xtask/src/main.rs:485-500): GICv2, cortex-a72, 272 MiB,
# ramfb for the framebuffer, xHCI keyboard+mouse.  `xtask run` attaches no SD
# image on virt, so neither does this.
set -uo pipefail
K="$1"; OUT="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT"
LOG="$OUT/$TAG-serial.log"
SOCK="/private/tmp/claude-501/sched01-$TAG-$$.sock"
rm -f "$LOG" "$SOCK"
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M \
  -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
  -display none -serial file:"$LOG" -no-reboot -kernel "$K" \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_timed.py" "$SOCK" "$OUT" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null; rm -f "$SOCK"
echo "run $TAG: rc=$RC ($(wc -l < "$LOG" | tr -d ' ') serial lines)"
