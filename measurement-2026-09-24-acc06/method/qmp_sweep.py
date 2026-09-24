#!/usr/bin/env python3
"""QMP driver: screendump at a fixed cadence, then quit.

usage: qmp_sweep.py SOCK OUTDIR PREFIX START_S STEP_S COUNT

Why a sweep and not a serial marker: QEMU's `-serial file:` chardev reaches
the file in blocks, so the \\AUTO\\ text console -- which `sh_tographic`
clears the moment the last AUTO program has terminated -- is long gone by the
time its own line shows up in the log (measured: both the SYSINFO marker and
the `desktop up` marker appeared at 15.1 s, 0.1 s apart, although the two
screens are seconds apart on the display). A fixed cadence does not depend on
the log at all.
"""
import json, os, socket, sys, time

sock_path, outdir, prefix, start_s, step_s, count = sys.argv[1:7]
start_s, step_s, count = float(start_s), float(step_s), int(count)

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

while time.time() - t0 < start_s:
    time.sleep(0.05)
for i in range(count):
    png = os.path.abspath(os.path.join(outdir, "%s-%02d.png" % (prefix, i)))
    r = cmd({"execute": "screendump", "arguments": {"filename": png, "format": "png"}})
    if 'error' in r:
        cmd({"execute": "screendump", "arguments": {"filename": png}})
    print("%.1fs -> %s" % (time.time() - t0, os.path.basename(png)))
    time.sleep(step_s)
cmd({"execute": "quit"})
