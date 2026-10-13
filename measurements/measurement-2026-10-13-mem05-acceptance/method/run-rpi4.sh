#!/usr/bin/env bash
# run-rpi4.sh <kernel8.img> <card.img> <tag> <SECONDS:PNG>...
# Headless QEMU raspi4b, `-append "${APPEND:-serial=on}"` (SER01), the card as SD (a
# private copy per run), serial to serial/<tag>-serial.log, QMP screendumps at
# fixed offsets into screendumps/, then `quit` (qmp_timed.py; the host has no
# `timeout`).
set -uo pipefail
K="$1"; SD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"; SER="$HERE/../serial"; mkdir -p "$OUT" "$SER"
LOG="$SER/$TAG-serial.log"
SDIR="$(mktemp -d s/a31.XXXXXX)"   # unix socket path < 104 B
CARD="$(dirname "$SD")/run-$TAG.img"
cp "$SD" "$CARD"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial file:"$LOG" -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
specs=(); for s in "$@"; do specs+=("${s%%:*}:$TAG-${s#*:}"); done
python3 "$HERE/qmp_timed.py" "$SDIR/qmp.sock" "$OUT" "${specs[@]}"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null; rm -f "$CARD"
echo "run $TAG: rc=$RC"
