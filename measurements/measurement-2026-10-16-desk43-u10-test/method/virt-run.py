#!/usr/bin/env python3
"""virt-run.py -- U10-TEST: one headless QEMU qemu-virt run (as run-virt.sh; no card; serial RX = console key on this board), driven from here.

usage: run.py KERNEL CARD TAG OUTDIR STEP [STEP ...]
  STEP = wait:<regex>[:<timeout s>]   wait until the serial log matches <regex>
       | sleep:<seconds>
       | shot:<name>                  QMP screendump -> OUTDIR/screendumps/<TAG>-<name>.png
       | send:<text>                  write <text> to the serial line (\\r for Return)
The card is a private APFS clone (cp -c) of CARD, deleted afterwards. Serial
goes through a socket chardev (so text can be sent) and is logged by this
script to OUTDIR/serial/<TAG>-serial.log. Boot arguments: -append serial=on
(SER01). No `timeout` on macOS: every wait has its own deadline; QEMU is
quit over QMP and killed if it lingers.
"""
import os, re, shutil, socket, subprocess, sys, tempfile, time, json

kernel, tag, outdir = sys.argv[1:4]
steps = sys.argv[4:]
os.makedirs(os.path.join(outdir, "serial"), exist_ok=True)
os.makedirs(os.path.join(outdir, "screendumps"), exist_ok=True)
sdir = os.path.relpath(tempfile.mkdtemp(prefix="u10.", dir=os.environ.get("SOCKDIR", ".")))  # unix socket path < 104 B
qmp_path, ser_path = os.path.join(sdir, "q"), os.path.join(sdir, "s")
log_path = os.path.join(outdir, "serial", f"{tag}-serial.log")
q = subprocess.Popen(["qemu-system-aarch64", "-M", "virt,gic-version=2", "-cpu", "cortex-a72", "-m", "272M",
                      "-device", "ramfb", "-device", "qemu-xhci", "-device", "usb-kbd", "-device", "usb-mouse",
                      "-display", "none", "-no-reboot", "-kernel", kernel,
                      "-chardev", f"socket,id=s0,path={ser_path},server=on,wait=on",
                      "-serial", "chardev:s0",
                      "-qmp", f"unix:{qmp_path},server=on,wait=off"])
def connect(p):
    for _ in range(300):
        try:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.connect(p); return s
        except OSError:
            time.sleep(0.1)
    sys.exit(f"cannot connect {p}")
ser = connect(ser_path); ser.setblocking(False)
qmp = connect(qmp_path); qf = qmp.makefile("rw", encoding="utf-8", newline="\n")
def qcmd(obj):
    qf.write(json.dumps(obj) + "\n"); qf.flush()
    while True:
        m = json.loads(qf.readline())
        if "return" in m or "error" in m:
            return m
qf.readline(); qcmd({"execute": "qmp_capabilities"})
log = open(log_path, "wb"); buf = bytearray(); t0 = time.time()
def pump(dt):
    end = time.time() + dt
    while time.time() < end:
        try:
            d = ser.recv(65536)
            if d:
                buf.extend(d); log.write(d); log.flush()
        except BlockingIOError:
            time.sleep(0.05)
rc = 0
for st in steps:
    kind, _, arg = st.partition(":")
    if kind == "wait":
        pat, _, to = arg.rpartition(":") if re.search(r":\d+$", arg) else (arg, "", "")
        to = float(to or 60); end = time.time() + to; rx = re.compile(pat.encode())
        while time.time() < end and not rx.search(buf):
            pump(0.2)
        ok = bool(rx.search(buf)); rc |= 0 if ok else 1
        print(f"t={time.time()-t0:.1f}s wait {pat!r}: {'ok' if ok else 'TIMEOUT'}")
    elif kind == "sleep":
        pump(float(arg))
    elif kind == "shot":
        png = os.path.abspath(os.path.join(outdir, "screendumps", f"{tag}-{arg}.png"))
        r = qcmd({"execute": "screendump", "arguments": {"filename": png, "format": "png"}})
        print(f"t={time.time()-t0:.1f}s shot {arg}: {json.dumps(r)}")
    elif kind == "send":
        ser.sendall(arg.encode().decode("unicode_escape").encode("latin-1"))
        print(f"t={time.time()-t0:.1f}s send {arg!r}")
pump(0.5)
try:
    qcmd({"execute": "quit"})
except Exception:
    pass
for _ in range(40):
    if q.poll() is not None:
        break
    time.sleep(0.25)
if q.poll() is None:
    q.kill()
log.close(); shutil.rmtree(sdir, ignore_errors=True)
print(f"run {tag}: rc={rc}")
sys.exit(rc)
