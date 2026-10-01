#!/usr/bin/env bash
# runboot.sh KERNEL8 IMG LOG TIMEOUT_S [DONE_MARKER]
# Boot QEMU raspi4b headless, stop as soon as DONE_MARKER (default
# "[desk] desktop up") or "[EXC]"/"[PANIC]" shows up in the serial log,
# or after TIMEOUT_S. Exit 0 either way; the log is the result.
set -uo pipefail
KERNEL8="$1"; IMG="$2"; LOG="$3"; TMO="$4"; DONE="${5:-[desk] desktop up}"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -kernel "$KERNEL8" \
  -drive file="$IMG",if=sd,format=raw &
QPID=$!
t0=$(date +%s)
while :; do
  sleep 0.3
  if [ -f "$LOG" ]; then
    if /usr/bin/grep -qF "$DONE" "$LOG" 2>/dev/null; then echo "runboot: done marker"; break; fi
    if /usr/bin/grep -qE '\[EXC\]|\[PANIC\]' "$LOG" 2>/dev/null; then sleep 2; echo "runboot: fault"; break; fi
  fi
  kill -0 "$QPID" 2>/dev/null || { echo "runboot: qemu exited"; break; }
  [ $(( $(date +%s) - t0 )) -ge "$TMO" ] && { echo "runboot: timeout"; break; }
done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
exit 0
