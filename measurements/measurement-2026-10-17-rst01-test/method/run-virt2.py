#!/usr/bin/env python3
"""run-virt.py KERNEL.img TAG -- qemu-virt smoke (xtask's virt arguments, -no-reboot): wait for "[desk] desktop up",
send Ctrl+Alt+Delete through QMP send-key to the usb-kbd, wait 5 s, record whether QEMU is still running, quit.
board-qemu-virt has no restart mechanism: expected "[reset] no restart mechanism on this board; ignored"."""
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
f.readline(); cmd({"execute": "qmp_capabilities"}); time.sleep(2)
print("send-key", cmd({"execute": "send-key", "arguments": {"hold-time": int(os.environ.get("HOLD","100")), "keys": [{"type": "qcode", "data": k} for k in ("ctrl", "alt", "delete")]}}))
time.sleep(5)
print("qemu still running after Ctrl+Alt+Del:", q.poll() is None)
cmd({"execute": "quit"}); q.wait(); os.rmdir(qd) if not os.listdir(qd) else None
data = open(log, "rb").read()
print("reset lines:", re.findall(rb"\[reset\][^\n]*", data), "EXC/PANIC:", len(re.findall(rb"\[EXC\]|PANIC|panicked", data)))
