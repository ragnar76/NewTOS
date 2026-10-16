#!/usr/bin/env bash
# run.sh <kernel8.img> <base.img> <tag> "<append>" -- one headless QEMU raspi4b boot (one QEMU at a time) on a per-run
# APFS clone of the base card; serial -> ../serial/<tag>-serial.log. Waits for the probe's end line and then
# BALL_WAIT seconds (default 45) for the autorun Ball demo, takes a QMP screendump into the scratch dir, quits QEMU,
# copies C:\BALL.LOG off the clone to ../serial/<tag>-BALL.LOG (read-only private mount), deletes the clone.
set -uo pipefail
K="$1"; BASE="$2"; TAG="$3"; APP="$4"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; SER="$HERE/../serial"; mkdir -p "$SER"
S="$(dirname "$BASE")"; RD="$(mktemp -d "$S/run-$TAG.XXXX")"; CARD="$RD/card.img"; LOG="$SER/$TAG-serial.log"
cp -c "$BASE" "$CARD"; rm -f "$LOG"
cd "$RD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" -kernel "$K" \
  -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
for _ in $(seq 1 240); do grep -q "MXPROBE: \(PASS\|FAIL\)" "$LOG" 2>/dev/null && break; kill -0 $QPID 2>/dev/null || break; sleep 1; done
for _ in $(seq 1 "${BALL_WAIT:-45}"); do kill -0 $QPID 2>/dev/null || break; sleep 1; done
python3 -I - qmp.sock "$S/$TAG.ppm" <<'PY'
import json, socket, sys, time
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f = s.makefile("rw")
def cmd(c, **a):
    f.write(json.dumps({"execute": c, **({"arguments": a} if a else {})}) + "\n"); f.flush()
    while True:
        r = json.loads(f.readline())
        if "return" in r or "error" in r: return r
f.readline(); cmd("qmp_capabilities"); print("screendump", cmd("screendump", filename=sys.argv[2])); cmd("quit")
PY
for _ in $(seq 1 20); do kill -0 $QPID 2>/dev/null || break; sleep 0.5; done; kill -9 $QPID 2>/dev/null; wait $QPID 2>/dev/null
DEV=$(hdiutil attach -readonly -imagekey diskimage-class=CRawDiskImage -nomount "$CARD" 2>/dev/null | head -1 | awk '{print $1}')
mkdir -p "$RD/mnt"; diskutil mount readOnly -mountPoint "$RD/mnt" "${DEV}s1" >/dev/null
cp "$RD/mnt/BALL.LOG" "$SER/$TAG-BALL.LOG" 2>/dev/null || echo "no BALL.LOG on the card"
diskutil unmount "$RD/mnt" >/dev/null; hdiutil detach "$DEV" >/dev/null 2>&1
cd "$HERE"; rm -rf "$RD"
echo "run $TAG done: $(grep -c . "$LOG") serial lines"
