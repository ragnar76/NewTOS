#!/usr/bin/env bash
# bootshot.sh <board rpi4|virt> <kernel> <sd-img-or-none> <outdir> <tag> <timeout>
set -uo pipefail
HERE=/Volumes/Daten/Development/PiRusTOS/measurement-2026-09-25-port03/method
B="$1"; K="$2"; SD="$3"; OUT="$4"; TAG="$5"; TMO="$6"
mkdir -p "$OUT"
LOG="$OUT/$TAG-serial.log"; SOCK="/private/tmp/claude-501/gui06-$TAG-$$.sock"
rm -f "$LOG" "$SOCK"
if [ "$B" = "rpi4" ]; then
  qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
    -kernel "$K" -drive file="$SD",if=sd,format=raw -qmp unix:"$SOCK",server,nowait &
else
  qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M \
    -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
    -display none -serial file:"$LOG" -no-reboot -kernel "$K" \
    -qmp unix:"$SOCK",server,nowait &
fi
QPID=$!
python3 "$HERE/qmp_multi.py" "$SOCK" "$LOG" "$OUT" "$TMO" "desktop up:$TAG.png:3"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null; rm -f "$SOCK"
echo "boot $TAG: rc=$RC"
