#!/usr/bin/env bash
# run.sh <tag> <xres> <yres> <shot-s ...> -- one headless QEMU raspi4b boot (release rpi4 kernel, -append serial=on) on an
# APFS clone (cp -c) of the base card; the bcm2835-fb geometry is set with -global bcm2835-fb.xres/yres (QEMU option only,
# the kernel reads it through the mailbox get-physical tag as on the Pi). Screendumps at the given seconds go to
# ../screendumps/<tag>-t<s>.png (BURST=n: then n back-to-back dumps <tag>-b<n>.png for the blink), serial to ../serial/<tag>.log; QEMU is quit after the last dump, the clone deleted.
set -uo pipefail
TAG="$1"; XR="$2"; YR="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SER="$HERE/../serial"; SHOT="$HERE/../screendumps"
RD="$(mktemp -d "$S/run-$TAG.XXXX")"; CARD="$RD/card.img"; LOG="$SER/$TAG.log"
cp -c "$S/base.img" "$CARD"; rm -f "$LOG"
cd "$RD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" -kernel "$S/kernel8-rpi4.img" \
  -global bcm2835-fb.xres="$XR" -global bcm2835-fb.yres="$YR" \
  -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!; T0=$(date +%s)
shot() { python3 -I - qmp.sock "$RD/shot.ppm" "$1" <<'PY'
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f = s.makefile("rw")
def cmd(c, **a):
    f.write(json.dumps({"execute": c, **({"arguments": a} if a else {})}) + "\n"); f.flush()
    while True:
        r = json.loads(f.readline())
        if "return" in r or "error" in r: return r
f.readline(); cmd("qmp_capabilities"); cmd("screendump", filename=sys.argv[2])
if sys.argv[3] == "quit": cmd("quit")
PY
}
for t in "$@"; do while [ $(( $(date +%s) - T0 )) -lt "$t" ]; do sleep 0.2; done; shot keep; sips -s format png "$RD/shot.ppm" --out "$SHOT/$TAG-t$t.png" >/dev/null 2>&1; done
n=0; while [ "$n" -lt "${BURST:-0}" ]; do n=$((n+1)); shot keep; sips -s format png "$RD/shot.ppm" --out "$SHOT/$TAG-b$n.png" >/dev/null 2>&1; \
  python3 -I -c 'import time; print(f"%.2f" % time.time())' >> "$SER/$TAG-burst-times.txt"; sleep 0.13; done   # BURST=n: n back-to-back dumps (blink)
shot quit
for _ in $(seq 1 20); do kill -0 $QPID 2>/dev/null || break; sleep 0.5; done; kill -9 $QPID 2>/dev/null; wait $QPID 2>/dev/null
cd "$HERE"; rm -rf "$RD"
echo "run $TAG: $(grep -ac . "$LOG") lines; EXC/PANIC: $(grep -Eac '\[EXC\]|\[crash\]|PANIC|panicked' "$LOG")"
