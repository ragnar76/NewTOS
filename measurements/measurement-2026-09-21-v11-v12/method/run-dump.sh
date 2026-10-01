#!/usr/bin/env bash
# run-dump.sh KERNEL8 IMG SERIAL_LOG MARKER TIMEOUT_S [SETTLE_S]
# Boots QEMU raspi4b headless on a fresh copy of IMG's content (IMG itself is
# written by the guest), waits for MARKER in the serial log, settles, kills.
set -uo pipefail
KERNEL8="$1"; IMG="$2"; LOG="$3"; MARKER="$4"; TMO="$5"; SETTLE="${6:-3}"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -kernel "$KERNEL8" \
  -drive file="$IMG",if=sd,format=raw &
QPID=$!
t0=$(date +%s); hit=0
while [ $(( $(date +%s) - t0 )) -lt "$TMO" ]; do
  if [ -f "$LOG" ] && /usr/bin/grep -q -F "$MARKER" "$LOG"; then hit=1; break; fi
  sleep 0.5
done
sleep "$SETTLE"
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
echo "marker hit=$hit after $(( $(date +%s) - t0 ))s"
[ "$hit" = 1 ]
