#!/usr/bin/env bash
# run_sweep.sh KERNEL8 IMG OUTDIR TAG START_S STEP_S COUNT
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KERNEL8="$1"; IMG="$2"; OUTDIR="$3"; TAG="$4"; START="$5"; STEP="$6"; COUNT="$7"
mkdir -p "$OUTDIR"
LOG="$OUTDIR/$TAG-serial.log"
SOCK="/private/tmp/claude-501/acc06-$TAG-$$.sock"
rm -f "$LOG" "$SOCK"
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -kernel "$KERNEL8" \
  -drive file="$IMG",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_sweep.py" "$SOCK" "$OUTDIR" "$TAG" "$START" "$STEP" "$COUNT"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SOCK"
echo "run $TAG: rc=$RC log=$LOG"
exit $RC
