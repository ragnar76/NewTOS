#!/usr/bin/env python3
"""time-cat.py <kernel> <card> <tag> -- one headless raspi4b boot (con01 kernel, per-run APFS clone); wall time (0.02 s poll) from the 2nd injector
'feed 0x1c' (Enter of 'cat long.txt') to the serial line 'END OF LONG.TXT'; serial -> ../serial/<tag>.log"""
import os, subprocess, sys, time, re
K, BASE, TAG = sys.argv[1:4]
HERE = os.path.dirname(os.path.abspath(__file__)); S = os.environ["S"]
log = os.path.join(HERE, "..", "serial", TAG + ".log"); card = os.path.join(S, "img", "t-" + TAG + ".img")
if os.path.exists(log): os.remove(log)
subprocess.check_call(["cp", "-c", BASE, card])
q = subprocess.Popen(["qemu-system-aarch64", "-M", "raspi4b", "-no-reboot", "-display", "none", "-append", "serial=on", "-kernel", K,
                      "-drive", "file=%s,if=sd,format=raw" % card, "-serial", "file:" + log])
t0 = time.time(); t1 = t2 = None
while time.time() - t0 < 90:
    time.sleep(0.02)
    try: d = open(log, "rb").read()
    except FileNotFoundError: continue
    if t1 is None and len(re.findall(rb"feed 0x1c at", d)) >= 2: t1 = time.time()
    if t2 is None and b"END OF LONG.TXT" in d: t2 = time.time(); break
q.kill(); q.wait(); os.remove(card)
print("%s: Enter(cat long) at %.2f s, END OF LONG.TXT at %.2f s, duration %.2f s" % (TAG, (t1 or 0) - t0, (t2 or 0) - t0, (t2 - t1) if (t1 and t2) else -1))
