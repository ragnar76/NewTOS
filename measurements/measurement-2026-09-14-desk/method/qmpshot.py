#!/usr/bin/env python3
"""qmpshot.py SOCKET OUT.ppm — connect to QEMU's QMP socket and screendump."""
import json, socket, sys, time
sock_path, out = sys.argv[1], sys.argv[2]
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
for _ in range(50):
    try:
        s.connect(sock_path); break
    except OSError:
        time.sleep(0.2)
else:
    print("no qmp socket"); sys.exit(1)
f = s.makefile('rw')
f.readline()                       # greeting
f.write(json.dumps({"execute": "qmp_capabilities"}) + "\n"); f.flush()
f.readline()
f.write(json.dumps({"execute": "screendump", "arguments": {"filename": out}}) + "\n"); f.flush()
print(f.readline().strip())
