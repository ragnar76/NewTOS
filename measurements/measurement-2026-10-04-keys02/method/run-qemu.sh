#!/usr/bin/env bash
# run-qemu.sh <kernel8.img> <sd.img> <outdir> <tag> <SECONDS:PNG...>
# Headless QEMU raspi4b with the card, QMP screendumps at the given offsets,
# then quit (recipe of measurement-2026-09-28-port07/method/run-ball.sh).
# The host has no `timeout`; qmp_timed.py sends `quit` after the last dump.
set -uo pipefail
K="$1"; SD="$2"; OUT="$3"; TAG="$4"; shift 4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT"
LOG="$OUT/$TAG-serial.log"
# A unix socket path must stay under 104 bytes; the scratch dir is too deep.
SDIR="$(mktemp -d /private/tmp/claude-501/k02.XXXXXX)"
SOCK="$SDIR/qmp.sock"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
  -kernel "$K" -drive file="$SD",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_timed.py" "$SOCK" "$OUT" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null; rm -f "$SOCK"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
