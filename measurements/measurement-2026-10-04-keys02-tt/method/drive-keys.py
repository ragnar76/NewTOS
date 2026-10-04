import sys, time, os
mode, fifo, out, hpid = sys.argv[1:5]; hpid = int(hpid)
logp = out + "/hatari.log"
def alive():
    try: os.kill(hpid, 0); return True
    except OSError: return False
def has(s):
    try: return s in open(logp, errors="replace").read()
    except OSError: return False
while not os.path.exists(fifo):
    time.sleep(0.5)
f = open(fifo, "w"); ilog = open(out + "/inject.log", "w")
t0 = time.time()
def send(ev):
    f.write("hatari-shortcut screenshot\n" if ev == "shot" else
            "hatari-shortcut quit\n" if ev == "quit" else "hatari-event %s\n" % ev); f.flush()
    ilog.write("%.3f s  %s\n" % (time.time() - t0, ev)); ilog.flush()
def waitfor(s, lim=600):
    n = 0
    while not has(s):
        if not alive(): print("hatari died waiting for", s); sys.exit(1)
        time.sleep(1); n += 1
        if n > lim: print("timeout waiting for", s); send("quit"); sys.exit(1)
    ilog.write("%.3f s  console: %s\n" % (time.time() - t0, s)); ilog.flush()
def run(steps):
    t1 = time.time()
    for at, ev in steps:
        d = at - (time.time() - t1)
        if d > 0: time.sleep(d)
        send(ev)
def tap(k, t0_, dur): return [(t0_, "keydown %d" % k), (t0_ + dur, "keyup %d" % k)]
if mode == "keys":
    waitfor("player 1 of 1"); time.sleep(5)
    ilog.write("--- sequence start (wall %.3f)\n" % (time.time() - t0)); ilog.flush()
    run([(0.0,"keydown 72"),(1.5,"keydown 77"),(2.5,"shot"),(3.0,"keyup 77"),(5.0,"keyup 72"),
         (7.0,"keydown 75"),(7.1,"keyup 75"),(8.0,"keydown 75"),(8.04,"keyup 75")]
        + tap(68,10.0,0.05))
else:
    waitfor("VERIFIED"); time.sleep(20); send("shot"); time.sleep(20); send("shot")
    ilog.write("--- quit sequence\n")
    run(tap(68,0,0.05))
# 'y' on both layouts, as DOOM's quit prompt ignores the other one
run(tap(21,3.0,0.05)+tap(44,4.0,0.05))
time.sleep(15); send("shot")
run(tap(1,2.0,0.05)+[(3.0,"keydown 56"),(3.05,"keydown 46"),(3.10,"keyup 46"),(3.15,"keyup 56")])
time.sleep(10); send("shot")
time.sleep(3); send("quit"); time.sleep(5)
