#!/usr/bin/env python3
# (copied unchanged from measurements/measurement-2026-10-13-mem05-acceptance/method/ for DESK43 U5a)
"""QMP driver: screendump at fixed wall-clock offsets, then quit.

usage: qmp_timed.py SOCK OUTDIR SPEC [SPEC ...]
  SPEC = SECONDS:PNGNAME
"""
import json, os, socket, sys, time

sock_path, outdir = sys.argv[1:3]
specs = []
for sp in sys.argv[3:]:
    a, b = sp.split(':', 1)
    specs.append((float(a), b))
specs.sort()

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

for at, png in specs:
    d = at - (time.time() - t0)
    if d > 0:
        time.sleep(d)
    path = os.path.abspath(os.path.join(outdir, png))
    r = cmd({"execute": "screendump", "arguments": {"filename": path, "format": "png"}})
    if 'error' in r:
        r = cmd({"execute": "screendump", "arguments": {"filename": path}})
    print("t=%.1fs -> %s %s" % (time.time() - t0, png, json.dumps(r)))
cmd({"execute": "quit"})
