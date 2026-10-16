#!/usr/bin/env bash
# run.sh <kernel8.img> <base.img> <tag> "<append>" -- one headless QEMU raspi4b boot on a per-run APFS clone of the
# base card (deleted afterwards); serial -> ../serial/<tag>-serial.log. Ends QEMU as soon as the probe prints its
# verdict (or after 240 s). Derived from measurements/measurement-2026-10-17-mem02/method/run.sh without the Ball part.
set -uo pipefail
K="$1"; BASE="$2"; TAG="$3"; APP="$4"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; SER="$HERE/../serial"; mkdir -p "$SER"
S="$(dirname "$BASE")"; RD="$(mktemp -d "$S/run-$TAG.XXXX")"; CARD="$RD/card.img"; LOG="$SER/$TAG-serial.log"
cp -c "$BASE" "$CARD"; rm -f "$LOG"
cd "$RD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" -kernel "$K" \
  -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" &
QPID=$!
for _ in $(seq 1 240); do grep -q "MXPROBE: \(PASS\|FAIL\)" "$LOG" 2>/dev/null && break; kill -0 $QPID 2>/dev/null || break; sleep 1; done
sleep 2; kill $QPID 2>/dev/null; sleep 1; kill -9 $QPID 2>/dev/null; wait $QPID 2>/dev/null
cd "$HERE"; rm -rf "$RD"
echo "run $TAG: $(grep -o 'MXPROBE: \(PASS\|FAIL\)' "$LOG" || echo 'no verdict'); $(grep -c . "$LOG") serial lines"
