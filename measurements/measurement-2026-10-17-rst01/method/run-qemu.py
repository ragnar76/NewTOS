#!/usr/bin/env python3
"""run-qemu.py KERNEL BASECARD TAG [--no-reboot] [--timeout S] [--stop REGEX@N] [--dump REGEX@N=DELAY:name] ...

One headless QEMU raspi4b run (-append "serial=on $APPEND_EXTRA") on an APFS clone (cp -c) of BASECARD, deleted afterwards.
Serial -> ../serial/TAG-serial.log, QMP screendumps -> ../screendumps/TAG-<name>.png, keyed to the N-th match of
REGEX in the serial log (+DELAY s). The run ends when QEMU exits on its own (that is what -no-reboot does on a guest
reset request), when --stop REGEX matched N times (then QEMU is quit over QMP), or after --timeout seconds.
Prints one summary line: how it ended, QEMU's exit status, elapsed time, and the number of boots in the log.
"""
import json, os, re, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
S = os.environ.get("S") or sys.exit("source env.sh first (S unset)")
a = sys.argv[1:]
kernel, base, tag = a[:3]
noreboot = False; timeout = 240.0; stop = None; dumps = []
i = 3
while i < len(a):
    if a[i] == "--no-reboot": noreboot = True; i += 1; continue
    if a[i] == "--timeout": timeout = float(a[i + 1]); i += 2; continue
    if a[i] == "--stop":
        rx, n = a[i + 1].rsplit("@", 1); stop = (rx.encode(), int(n)); i += 2; continue
    if a[i] == "--dump":
        m = re.match(r"^(.*)@(\d+)=([0-9.]+):(.*)$", a[i + 1]); assert m, a[i + 1]
        dumps.append([m.group(1).encode(), int(m.group(2)), float(m.group(3)), m.group(4), False]); i += 2; continue
    sys.exit("unknown arg " + a[i])

serial_dir = os.path.join(HERE, "..", "serial"); shot_dir = os.path.join(HERE, "..", "screendumps")
kernel = os.path.abspath(kernel); base = os.path.abspath(base)
os.makedirs(serial_dir, exist_ok=True); os.makedirs(shot_dir, exist_ok=True)
log = os.path.join(serial_dir, tag + "-serial.log")
card = os.path.join(S, "img", "run-" + tag + ".img")
if os.path.exists(log): os.remove(log)
subprocess.check_call(["cp", "-c", base, card])
os.chdir(S)  # the QMP socket path must stay < 104 bytes: relative to $S
qd = os.path.basename(tempfile.mkdtemp(dir=S, prefix="q."))
sock = os.path.join(qd, "qmp.sock")
cmd = ["qemu-system-aarch64", "-M", "raspi4b", "-display", "none", "-append", " ".join(["serial=on"] + os.environ.get("APPEND_EXTRA", "").split()),
       "-kernel", kernel, "-drive", "file=%s,if=sd,format=raw" % card, "-serial", "file:" + log,
       "-qmp", "unix:%s,server=on,wait=off" % sock]
if noreboot: cmd.insert(3, "-no-reboot")
t0 = time.time()
q = subprocess.Popen(cmd)

f = None
def qmp(o):
    global f
    if f is None:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        for _ in range(100):
            try: s.connect(sock); break
            except OSError: time.sleep(0.2)
        f = s.makefile("rw", encoding="utf-8", newline="\n"); f.readline()
        qmp({"execute": "qmp_capabilities"})
    f.write(json.dumps(o) + "\n"); f.flush()
    while True:
        l = f.readline()
        if not l: return {"error": "closed"}
        m = json.loads(l)
        if "return" in m or "error" in m: return m

ended = "timeout"
while time.time() - t0 < timeout:
    rc = q.poll()
    if rc is not None: ended = "qemu-exited-by-itself"; break
    try: data = open(log, "rb").read()
    except OSError: data = b""
    for d in dumps:
        if not d[4] and len(re.findall(d[0], data)) >= d[1]:
            time.sleep(d[2]); d[4] = True
            if q.poll() is None:
                p = os.path.abspath(os.path.join(shot_dir, "%s-%s.png" % (tag, d[3])))
                r = qmp({"execute": "screendump", "arguments": {"filename": p, "format": "png"}})
                print("t=%.1fs dump %s %s" % (time.time() - t0, d[3], json.dumps(r)), flush=True)
    if stop and len(re.findall(stop[0], data)) >= stop[1]:
        ended = "stop-marker"; break
    time.sleep(0.25)
if q.poll() is None:
    try: qmp({"execute": "quit"})
    except Exception: pass
    for _ in range(40):
        if q.poll() is not None: break
        time.sleep(0.25)
    if q.poll() is None: q.kill()
rc = q.wait()
os.remove(card)
for n in os.listdir(qd): os.remove(os.path.join(qd, n))
os.rmdir(qd)
data = open(log, "rb").read()
boots = len(re.findall(rb"NewTOS booting on", data))
print("run %s: ended=%s qemu_rc=%s elapsed=%.1fs no_reboot=%s boots=%d reset_lines=%d EXC/PANIC=%d" % (
    tag, ended, rc, time.time() - t0, noreboot, boots, len(re.findall(rb"\[reset\]", data)),
    len(re.findall(rb"\[EXC\]|PANIC|panicked", data))), flush=True)
