#!/usr/bin/env bash
# run-ball.sh <kernel8.img> <sd.img> <outdir> <tag> <spec...>
# Boot QEMU raspi4b headless with the card, screendump at the given offsets,
# quit, then copy C:\BALL.LOG out of the card.
set -uo pipefail
K="$1"; SD="$2"; OUT="$3"; TAG="$4"; shift 4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT"
LOG="$OUT/$TAG-serial.log"
SOCK="/private/tmp/claude-501/port07-$TAG-$$.sock"
rm -f "$LOG" "$SOCK"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
  -kernel "$K" -drive file="$SD",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_timed.py" "$SOCK" "$OUT" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null; rm -f "$SOCK"
echo "run $TAG: rc=$RC"
