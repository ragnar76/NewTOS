#!/usr/bin/env bash
# run-qemu.sh KERNEL8 IMG LOG TIMEOUT_S  -- headless raspi4b, stops at "desktop up" or timeout
set -uo pipefail
K="$1"; IMG="$2"; LOG="$3"; TMO="$4"
SOCK="/private/tmp/claude-501/a62-$$.sock"
rm -f "$LOG" "$SOCK"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" -kernel "$K" \
  -drive file="$IMG",if=sd,format=raw -qmp unix:"$SOCK",server,nowait &
Q=$!
for i in $(seq 1 "$TMO"); do
  sleep 1
  if /usr/bin/grep -q "desktop up" "$LOG" 2>/dev/null; then sleep 2; break; fi
  kill -0 $Q 2>/dev/null || break
done
kill $Q 2>/dev/null; sleep 1; kill -9 $Q 2>/dev/null; wait $Q 2>/dev/null
rm -f "$SOCK"
echo "run done: $(wc -l < "$LOG") lines"
