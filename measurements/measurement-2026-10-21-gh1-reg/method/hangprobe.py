#!/usr/bin/env python3
"""hangprobe.py KERNEL.img CARD TAG LOG WAIT_MARK_REGEX SETTLE_S N -- boot headless like run.sh, wait for the serial regex, settle, then sample the guest PC/ELR/SP N times via QMP human-monitor `info registers` (cpu 0), print them."""
import json, os, re, socket, subprocess, sys, time
K, CARD, TAG, LOG, RX, SETTLE, N = sys.argv[1:8]; SETTLE = float(SETTLE); N = int(N)
rd = os.path.dirname(CARD); sock = "qmp-%s.sock" % TAG; os.chdir(rd)
if os.path.exists(LOG): os.remove(LOG)
q = subprocess.Popen(["qemu-system-aarch64", "-M", "raspi4b", "-no-reboot", "-display", "none", "-append", "serial=on", "-kernel", K,
                      "-drive", "file=%s,if=sd,format=raw" % CARD, "-serial", "file:" + LOG, "-qmp", "unix:%s,server=on,wait=off" % sock])
t0 = time.time()
while not os.path.exists(sock) and time.time() - t0 < 30: time.sleep(0.2)
s = socket.socket(socket.AF_UNIX); s.connect(sock); f = s.makefile("rw"); f.readline()
def cmd(c, a=None):
    f.write(json.dumps({"execute": c, **({"arguments": a} if a else {})}) + "\n"); f.flush()
    while True:
        r = json.loads(f.readline())
        if "return" in r or "error" in r: return r
cmd("qmp_capabilities")
while time.time() - t0 < 300:
    try: txt = open(LOG, errors="replace").read()
    except FileNotFoundError: txt = ""
    if re.search(RX, txt): break
    time.sleep(0.3)
time.sleep(SETTLE)
for i in range(N):
    r = cmd("human-monitor-command", {"command-line": "info registers"})["return"]
    keep = r.splitlines() if os.environ.get("FULL") else [l for l in r.splitlines() if re.match(r"\s*(PC|ELR_EL1|SP_EL0|SP_EL1|X30|SPSR_EL1|CPSR|PSTATE|ESR_EL1)", l)]
    print("sample", i, " | ".join(l.strip() for l in keep)[:2000], flush=True); time.sleep(0.7)
for xc in filter(None, os.environ.get("XCMDS", "").split(";")):
    print("mon", xc, "->", cmd("human-monitor-command", {"command-line": xc})["return"].strip(), flush=True)
cmd("quit"); time.sleep(1); q.kill() if q.poll() is None else None
