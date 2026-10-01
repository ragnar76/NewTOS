#!/usr/bin/env python3
"""QMP driver: wait for successive serial markers, screendump at each, then quit.

usage: qmp_multi.py SOCK SERIAL_LOG OUTDIR TIMEOUT_S SPEC [SPEC ...]
  SPEC = MARKER:PNGNAME[:SETTLE_S[:stop]]   (markers are awaited in order)

A fourth field `stop` pauses the VM (QMP `stop`) before the screendump and
resumes it (`cont`) afterwards. Needed for a screen that only exists for a
few milliseconds -- the \AUTO\ text console, which `sh_tographic` clears as
soon as the last AUTO program has terminated (DESK28).
"""
import json, os, socket, sys, time

sock_path, log_path, outdir, timeout_s = sys.argv[1:5]
timeout_s = float(timeout_s)
specs = []
for sp in sys.argv[5:]:
    # `|` as the field separator when the marker itself contains a colon
    # ("[auto] running C:\\AUTO\\..."); `:` otherwise, as before.
    parts = sp.split('|') if '|' in sp else sp.split(':')
    specs.append((parts[0], parts[1], float(parts[2]) if len(parts) > 2 and parts[2] else 0.0,
                  len(parts) > 3 and parts[3] == 'stop'))

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

def shot(png):
    path = os.path.abspath(os.path.join(outdir, png))
    r = cmd({"execute": "screendump", "arguments": {"filename": path, "format": "png"}})
    if 'error' in r:
        r = cmd({"execute": "screendump", "arguments": {"filename": path}})
    return r

pos = 0
ok = 0
for marker, png, settle, pause in specs:
    hit = False
    while time.time() - t0 < timeout_s:
        try:
            data = open(log_path, 'rb').read()
        except FileNotFoundError:
            data = b''
        i = data.find(marker.encode(), pos)
        if i >= 0:
            pos = i + len(marker)
            hit = True
            break
        time.sleep(0.05)
    tm = time.time() - t0
    if not hit:
        print("marker %r NOT FOUND after %.1fs" % (marker, tm)); break
    if pause:
        cmd({"execute": "stop"})
    if settle:
        time.sleep(settle)
    r = shot(png)
    if pause:
        cmd({"execute": "cont"})
    print("marker %r found at %.1fs -> %s %s" % (marker, tm, png, json.dumps(r)))
    ok += 1
cmd({"execute": "quit"})
sys.exit(0 if ok == len(specs) else 2)
