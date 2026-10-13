#!/usr/bin/env bash
# smoke.sh <kernel8.img> <tag> <append> [seconds]
# QEMU raspi4b boot without a card, serial to serial/<tag>-serial.log, one QMP
# screendump at <seconds> (default 40), then quit. Socket in a short temp dir
# (unix socket path < 104 B; the host has no `timeout`).
set -uo pipefail
K="$1"; TAG="$2"; APP="$3"; SECS="${4:-40}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"; SER="$HERE/../serial"; mkdir -p "$OUT" "$SER"
LOG="$SER/$TAG-serial.log"; rm -f "$LOG"
SDIR="$(mktemp -d "${SOCKBASE:-/tmp/m5j}/s.XXXX")"   # SOCKBASE: short symlink to the job scratch dir
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" \
  -kernel "$K" -serial file:"$LOG" -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/qmp_timed.py" "$SDIR/qmp.sock" "$OUT" "$SECS:$TAG-t$SECS"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
