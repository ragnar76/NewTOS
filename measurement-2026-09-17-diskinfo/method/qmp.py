#!/usr/bin/env python3
"""Minimal QMP client: wait for a marker in the serial log, screendump, quit.

usage: qmp.py SOCK SERIAL_LOG MARKER TIMEOUT_S SETTLE_S PNG_OUT
"""
import json, os, socket, sys, time

sock_path, log_path, marker, timeout_s, settle_s, png_out = sys.argv[1:7]
timeout_s = float(timeout_s); settle_s = float(settle_s)

# Wait for the QMP socket to appear.
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

f.readline()                      # greeting
cmd({"execute": "qmp_capabilities"})

hit = False
t0 = time.time()
while time.time() - t0 < timeout_s:
    try:
        with open(log_path, 'rb') as lf:
            if marker.encode() in lf.read():
                hit = True
                break
    except FileNotFoundError:
        pass
    time.sleep(0.5)

print("marker %r %s after %.1fs" % (marker, "FOUND" if hit else "NOT FOUND", time.time() - t0))
time.sleep(settle_s)

r = cmd({"execute": "screendump",
         "arguments": {"filename": os.path.abspath(png_out), "format": "png"}})
if 'error' in r:
    # Older QEMU: no `format` argument -> PPM under the same name.
    r = cmd({"execute": "screendump", "arguments": {"filename": os.path.abspath(png_out)}})
print("screendump:", json.dumps(r))

cmd({"execute": "quit"})
sys.exit(0 if hit else 2)
