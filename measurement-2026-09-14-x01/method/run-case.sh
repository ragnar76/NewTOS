#!/usr/bin/env bash
# run-case.sh KERNEL8 IMG OUTDIR TAG MARKER TIMEOUT_S SETTLE_S
#
# Boot one card headless on QEMU raspi4b, wait for MARKER on the serial line,
# take a QMP screendump, quit. Leaves OUTDIR/TAG-BOOT-SERIAL.txt and
# OUTDIR/TAG.png. Never blocks forever: the emulator is killed on timeout.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KERNEL8="$1"; IMG="$2"; OUTDIR="$3"; TAG="$4"; MARKER="$5"; TMO="${6:-180}"; SETTLE="${7:-3}"
mkdir -p "$OUTDIR"
LOG="$OUTDIR/$TAG-BOOT-SERIAL.txt"
SOCK="$OUTDIR/$TAG.qmp"   # the recorded runs used /tmp/x01-$TAG.qmp; kept out
                          # of the shared /tmp since, so parallel jobs cannot collide
PNG="$OUTDIR/$TAG.png"
rm -f "$LOG" "$SOCK" "$PNG"

"$HERE/boot-qmp.sh" "$KERNEL8" "$IMG" "$LOG" "$SOCK" &
QPID=$!
python3 "$HERE/qmp.py" "$SOCK" "$LOG" "$MARKER" "$TMO" "$SETTLE" "$PNG"
RC=$?
# Give QEMU a moment to flush and exit on its own, then make sure it is gone.
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null
wait "$QPID" 2>/dev/null
rm -f "$SOCK"
echo "run-case $TAG: qmp rc=$RC  log=$LOG  png=$PNG"
exit $RC
