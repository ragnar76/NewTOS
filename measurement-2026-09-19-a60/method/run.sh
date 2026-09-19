#!/usr/bin/env bash
# run.sh KERNEL8 IMG OUTDIR TAG WAIT_S  -- boot headless raspi4b, wait, screendump, quit
set -uo pipefail
KERNEL8="$1"; IMG="$2"; OUTDIR="$3"; TAG="$4"; WAIT="$5"
mkdir -p "$OUTDIR"
LOG="$OUTDIR/$TAG-serial.log"
SOCK="/private/tmp/claude-501/a60-$TAG.sock"
rm -f "$LOG" "$SOCK"
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -kernel "$KERNEL8" \
  -drive file="$IMG",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait &
QPID=$!
python3 - "$SOCK" "$OUTDIR/$TAG-screen.png" "$WAIT" <<'PY'
import json, os, socket, sys, time
sock_path, png, wait = sys.argv[1], sys.argv[2], float(sys.argv[3])
t0 = time.time()
while not os.path.exists(sock_path):
    if time.time() - t0 > 30: sys.exit("qmp: socket never appeared")
    time.sleep(0.2)
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
for _ in range(100):
    try: s.connect(sock_path); break
    except OSError: time.sleep(0.2)
f = s.makefile('rw', encoding='utf-8', newline='\n')
def cmd(o):
    f.write(json.dumps(o)+'\n'); f.flush()
    while True:
        m = json.loads(f.readline())
        if 'return' in m or 'error' in m: return m
f.readline(); cmd({"execute":"qmp_capabilities"})
time.sleep(wait)
r = cmd({"execute":"screendump","arguments":{"filename":os.path.abspath(png),"format":"png"}})
if 'error' in r: r = cmd({"execute":"screendump","arguments":{"filename":os.path.abspath(png)}})
print("screendump:", json.dumps(r))
cmd({"execute":"quit"})
PY
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SOCK"
echo "run $TAG: rc=$RC log=$LOG"
