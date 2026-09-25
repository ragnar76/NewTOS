#!/usr/bin/env python3
"""Turn the emulated USB mouse's wheel through QMP.

usage: qmp_wheel.py SOCK SERIAL_LOG MARKER TIMEOUT_S UP_COUNT DOWN_COUNT

Waits for MARKER in the serial log, then sends UP_COUNT `wheel-up` and
DOWN_COUNT `wheel-down` button events with `input-send-event` (QEMU's
InputButton values `wheel-up`/`wheel-down`), each as a press + release pair,
and finally quits the guest.

The events go into QEMU's input subsystem, which is what the `usb-mouse`
device turns into a real 4-byte HID boot report on its interrupt endpoint --
so everything from the xHCI transfer down is the product code's own work.
"""
import json
import os
import socket
import sys
import time

sock_path, log_path, marker, timeout_s, n_up, n_dn = sys.argv[1:7]
timeout_s = float(timeout_s)
n_up, n_dn = int(n_up), int(n_dn)

t0 = time.time()
while not os.path.exists(sock_path):
    if time.time() - t0 > 30:
        sys.exit("qmp: socket never appeared")
    time.sleep(0.2)
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
for _ in range(100):
    try:
        s.connect(sock_path)
        break
    except OSError:
        time.sleep(0.2)
else:
    sys.exit("qmp: cannot connect")
f = s.makefile('rw', encoding='utf-8', newline='\n')


def cmd(obj):
    f.write(json.dumps(obj) + "\n")
    f.flush()
    while True:
        line = f.readline()
        if not line:
            sys.exit("qmp: connection closed")
        msg = json.loads(line)
        if 'return' in msg or 'error' in msg:
            return msg


f.readline()                      # greeting
cmd({"execute": "qmp_capabilities"})

# Wait for the guest to be ready.
t0 = time.time()
while True:
    try:
        if marker in open(log_path, errors='replace').read():
            break
    except FileNotFoundError:
        pass
    if time.time() - t0 > timeout_s:
        sys.exit(f"marker {marker!r} NOT FOUND after {timeout_s}s")
    time.sleep(0.2)
print(f"marker {marker!r} found at {time.time() - t0:.1f}s")
time.sleep(2.0)                   # let the desktop settle and start polling


def wheel(button, n):
    for i in range(n):
        r = cmd({
            "execute": "input-send-event",
            "arguments": {"events": [
                {"type": "btn", "data": {"down": True, "button": button}},
                {"type": "btn", "data": {"down": False, "button": button}},
            ]},
        })
        print(f"{button} #{i + 1}: {json.dumps(r)}")
        time.sleep(0.4)


wheel("wheel-up", n_up)
wheel("wheel-down", n_dn)
time.sleep(2.0)
cmd({"execute": "quit"})
