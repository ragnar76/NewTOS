#!/usr/bin/env bash
# run-blink.sh <kernel8.img> <card.img> <tag> -- the cursor-blink check: boot (serial=on) on an APFS clone of the card,
# at 28 s take eight QMP screendumps 0.25 s apart (scratch $S/shots/<tag>-b<n>.png), quit, delete the clone. The card's
# AUTO probe must end with a visible, FLASHING cursor (vt52prb.c with Cursconf(3) instead of Cursconf(2)).
set -uo pipefail
K="$1"; BASE="$2"; TAG="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"; SER="$HERE/../serial"; SHOT="$S/shots"; mkdir -p "$SER" "$SHOT"
RD="$(mktemp -d "$S/run-$TAG.XXXX")"; CARD="$RD/card.img"; LOG="$SER/$TAG.log"
cp -c "$BASE" "$CARD"; rm -f "$LOG"; cd "$RD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" -kernel "$K" \
  -drive file="$CARD",if=sd,format=raw -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!; sleep "${DELAY:-28}"
python3 -I - qmp.sock "$RD" <<'PY'
import json, socket, sys, time
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f = s.makefile("rw")
def cmd(c, **a):
    f.write(json.dumps({"execute": c, **({"arguments": a} if a else {})}) + "\n"); f.flush()
    while True:
        r = json.loads(f.readline())
        if "return" in r or "error" in r: return r
f.readline(); cmd("qmp_capabilities")
for i in range(8):
    cmd("screendump", filename=f"{sys.argv[2]}/b{i}.ppm"); time.sleep(0.25)
cmd("quit")
PY
for _ in $(seq 1 20); do kill -0 $QPID 2>/dev/null || break; sleep 0.5; done; kill -9 $QPID 2>/dev/null; wait $QPID 2>/dev/null
for i in 0 1 2 3 4 5 6 7; do sips -s format png "$RD/b$i.ppm" --out "$SHOT/$TAG-b$i.png" >/dev/null 2>&1; done
cd "$HERE"; rm -rf "$RD"
python3 -I - "$SHOT" "$TAG" <<'PY'
import sys
from PIL import Image
for i in range(8):
    im = Image.open(f"{sys.argv[1]}/{sys.argv[2]}-b{i}.png").convert("RGB")
    print(f"b{i}: cursor cell (40,28) centre pixel {im.getpixel((40 * 8 + 3, 28 * 16 + 8))}")
PY
