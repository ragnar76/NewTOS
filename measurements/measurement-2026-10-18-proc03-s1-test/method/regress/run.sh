#!/usr/bin/env bash
# run.sh <kernel8.img> <base.img> <tag> "<append>" "<end-regex>" <max-s> [shot-s ...] -- one headless QEMU raspi4b boot on a
# per-run APFS clone (cp -c) of the base card (one QEMU at a time); serial -> ../../serial/<tag>.log. Waits up to <max-s>
# for <end-regex>, then 3 s; screendumps at the given seconds after start (../../screendumps is NOT used: dumps go to the
# scratch dir $S/shots, sha256 recorded by the caller). Adapted from measurement-2026-10-17-rst01-test/method/m113/api-run.sh.
set -uo pipefail
K="$1"; BASE="$2"; TAG="$3"; APP="$4"; END="$5"; MAX="$6"; shift 6
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/../env.sh"; SER="$HERE/../../serial"; SHOT="$S/shots"; mkdir -p "$SER" "$SHOT"
RD="$(mktemp -d "$S/run-$TAG.XXXX")"; CARD="$RD/card.img"; LOG="$SER/$TAG.log"
cp -c "$BASE" "$CARD"; rm -f "$LOG"
cd "$RD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "$APP" -kernel "$K" \
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
for t in "$@"; do while [ $(( $(date +%s) - T0 )) -lt "$t" ]; do sleep 1; done; shot keep; sips -s format png "$RD/shot.ppm" --out "$SHOT/$TAG-t$t.png" >/dev/null 2>&1; done
while [ $(( $(date +%s) - T0 )) -lt "$MAX" ]; do grep -Eaq "$END" "$LOG" 2>/dev/null && break; kill -0 $QPID 2>/dev/null || break; sleep 1; done
sleep 3; shot keep; sips -s format png "$RD/shot.ppm" --out "$SHOT/$TAG-final.png" >/dev/null 2>&1; shot quit
for _ in $(seq 1 20); do kill -0 $QPID 2>/dev/null || break; sleep 0.5; done; kill -9 $QPID 2>/dev/null; wait $QPID 2>/dev/null
cd "$HERE"; rm -rf "$RD"
echo "run $TAG: $(grep -ac . "$LOG") lines; end: $(grep -Eac "$END" "$LOG"); EXC/PANIC: $(grep -Eac '\[EXC\]|\[crash\]|PANIC|panicked' "$LOG")"
