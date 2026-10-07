#!/usr/bin/env python3
"""(FS08-CPUCLK copy of measurement-2026-10-07-fat-curcl/method/qmp_on_marker.py;
only change: the post-marker delay comes from $MARK_DELAY, default 0.5 s.)

QMP driver: take a screendump each time MARKER appears once more in the
serial log, quit after N dumps (or after TIMEOUT seconds).

usage: qmp_on_marker.py SOCK SERIAL_LOG OUTDIR MARKER N TIMEOUT PREFIX
  dumps OUTDIR/PREFIX-1.png .. PREFIX-N.png, then sends `quit`.
"""
import json, os, socket, sys, time

sock_path, log, outdir, marker, n, timeout, prefix = sys.argv[1:8]
n = int(n); timeout = float(timeout)
t0 = time.time()
while not os.path.exists(sock_path):
    if time.time() - t0 > 30:
        sys.exit("qmp: socket never appeared")
    time.sleep(0.2)
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
for _ in range(100):
    try:
        s.connect(sock_path); break
    except OSError:
        time.sleep(0.2)
else:
    sys.exit("qmp: cannot connect")
f = s.makefile('rw', encoding='utf-8', newline='\n')
def cmd(obj):
    f.write(json.dumps(obj) + '\n'); f.flush()
    while True:
        line = f.readline()
        if not line:
            sys.exit("qmp: connection closed")
        msg = json.loads(line)
        if 'return' in msg or 'error' in msg:
            return msg
f.readline()
cmd({"execute": "qmp_capabilities"})

done = 0
while done < n and time.time() - t0 < timeout:
    try:
        seen = open(log, 'rb').read().count(marker.encode())
    except OSError:
        seen = 0
    if seen > done:
        time.sleep(float(os.environ.get("MARK_DELAY", "0.5")))  # let the page finish rendering
        done += 1
        path = os.path.abspath(os.path.join(outdir, "%s-%d.png" % (prefix, done)))
        r = cmd({"execute": "screendump", "arguments": {"filename": path, "format": "png"}})
        print("t=%.1fs marker #%d -> %s %s" % (time.time() - t0, done, path, json.dumps(r)))
        continue
    time.sleep(0.2)
if done < n:
    print("TIMEOUT after %.0f s, %d of %d markers" % (time.time() - t0, done, n))
time.sleep(3)
cmd({"execute": "quit"})
