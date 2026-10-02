#!/usr/bin/env bash
# run-ser01.sh <kernel8.img> <sd.img> <outdir> <tag> <APPEND|-> <SECONDS:PNG...>
# Headless QEMU raspi4b with a private copy of the card, QMP screendumps at
# the given offsets, then quit (recipe of
# measurement-2026-10-02-port06-keys/method/run-doom.sh). APPEND is passed as
# `-append APPEND` (the stand-in for cmdline.txt); "-" passes no -append at
# all, i.e. the boot arguments are empty. The host has no `timeout`;
# qmp_timed.py sends `quit` after the last dump.
set -uo pipefail
K="$1"; SD="$2"; OUT="$3"; TAG="$4"; APP="$5"; shift 5
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT/serial" "$OUT/screendumps"
LOG="$OUT/serial/$TAG-serial.log"
SDIR="$(mktemp -d /private/tmp/claude-501/ser01.XXXXXX)"
SOCK="$SDIR/qmp.sock"
cp "$SD" "$SDIR/sd.img"
rm -f "$LOG"
EXTRA=()
[ "$APP" != "-" ] && EXTRA=(-append "$APP")
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
  -kernel "$K" -drive file="$SDIR/sd.img",if=sd,format=raw ${EXTRA[@]+"${EXTRA[@]}"} \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 "$HERE/qmp_timed.py" "$SOCK" "$OUT/screendumps" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SOCK" "$SDIR/sd.img"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC log=$(wc -c < "$LOG") bytes"
