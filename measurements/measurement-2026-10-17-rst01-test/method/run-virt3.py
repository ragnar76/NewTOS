#!/usr/bin/env python3
"""run-virt3.py KERNEL.img TAG -- qemu-virt USB-HID key matrix (derived from the RST01 run-virt.py): after "[desk] desktop up", send a
list of QMP send-key combinations 2 s apart and print the "[reset]" lines the serial log gained after each. Expected: only combos with
Ctrl+Alt+Delete (no left Shift) log "[reset] keyboard ... requested" (Warm; +right Shift = Cold) and then "no restart mechanism".
Hold 150 ms."""
import json, os, re, socket, subprocess, sys, tempfile, time
HERE = os.path.dirname(os.path.abspath(__file__)); S = os.environ["S"]
kernel, tag = os.path.abspath(sys.argv[1]), sys.argv[2]
log = os.path.join(HERE, "..", "serial", tag + "-serial.log")
os.chdir(S); qd = os.path.basename(tempfile.mkdtemp(dir=S, prefix="q.")); sock = qd + "/qmp.sock"
q = subprocess.Popen(["qemu-system-aarch64", "-M", "virt,gic-version=2", "-cpu", "cortex-a72", "-m", "272M",
    "-device", "ramfb", "-device", "qemu-xhci", "-device", "usb-kbd", "-device", "usb-mouse", "-display", "none",
    "-serial", "file:" + log, "-no-reboot", "-kernel", kernel, "-qmp", "unix:%s,server=on,wait=off" % sock])
t0 = time.time()
while time.time() - t0 < 120 and b"desktop up" not in (open(log, "rb").read() if os.path.exists(log) else b""):
    time.sleep(0.5)
s = socket.socket(socket.AF_UNIX); s.connect(sock); f = s.makefile("rw")
def cmd(o):
    f.write(json.dumps(o) + "\n"); f.flush()
    while True:
        m = json.loads(f.readline())
        if "return" in m or "error" in m: return m
f.readline(); cmd({"execute": "qmp_capabilities"}); time.sleep(3)
combos = [("delete",), ("ctrl", "delete"), ("alt", "delete"), ("shift", "ctrl", "alt", "delete"), ("ctrl", "alt", "shift_r", "delete"),
          ("a",), ("shift", "a"), ("shift_r", "a"), ("ctrl", "alt", "delete")]
for c in combos:
    n0 = len(re.findall(rb"\[reset\]", open(log, "rb").read()))
    cmd({"execute": "send-key", "arguments": {"hold-time": 150, "keys": [{"type": "qcode", "data": k} for k in c]}})
    time.sleep(2)
    d = open(log, "rb").read(); n1 = len(re.findall(rb"\[reset\]", d))
    print("+".join(c), "->", "reset lines +%d" % (n1 - n0), re.findall(rb"\[reset\] keyboard[^\n]*", d)[-1:] if n1 > n0 else "", flush=True)
print("qemu still running:", q.poll() is None)
cmd({"execute": "quit"}); q.wait(); os.rmdir(qd) if not os.listdir(qd) else None
d = open(log, "rb").read(); print("EXC/PANIC:", len(re.findall(rb"\[EXC\]|PANIC|panicked", d)))
