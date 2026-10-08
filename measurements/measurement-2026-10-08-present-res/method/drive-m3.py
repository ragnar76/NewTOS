#!/usr/bin/env python3
"""drive-m3.py -- INPUT01 copy (measurement-2026-10-08-ofam-freeze/method/drive-m3.py)
with one addition: --burst NAME N INTERVAL -- when "[m3] MARK NAME" appears on
the serial line, N screendumps TAG-NAME-bNN.png are taken INTERVAL seconds
apart (host clock, independent of the guest), so the screen is sampled while
the guest is busy (zView loading its codecs) whether or not the input pump runs.

Original: drive.py of measurement-2026-10-06-ldg-m1 (serial + QMP
driver) plus one addition for the TASK/LDG-M3 injector runs: every serial
line "[m3] MARK <NAME> ..." (printed by the scratch injector,
apply-injector-m3.py) is followed, after --mark-delay seconds (default 1.0),
by a QMP screendump TAG-<NAME>.png.

Original description of drive.py (ARCH03 0c crash runs):

usage: drive.py SERIAL_SOCK QMP_SOCK OUTDIR TAG [--answer] [--end REGEX]
                [--end-delay S] [--timeout S]

* Reads the guest serial line (QEMU `-serial unix:...,server=on,wait=on`)
  and writes it verbatim to OUTDIR/../serial/TAG-serial.log.
* Every time the crash prompt "*** Press any key to continue ***" appears,
  it waits 0.8 s and takes a QMP screendump TAG-crashN.png -- the report is
  on screen while the prompt waits.
* --answer: then sends one Space over the serial line (qemu-virt is the
  serial-console profile: serial RX = console key). Without it the scratch
  IKBD injector answers (board-rpi4), 2 s after the prompt.
* --stamp (ARCH03-4): also write OUTDIR/../serial/TAG-stamped.log, every
  serial line prefixed with the host time in seconds since the start (for
  wall-clock timing of probe parts; QEMU numbers are only indicative).
* When REGEX (default: the desktop-up line) has matched, waits END_DELAY s,
  takes TAG-end.png and quits QEMU. The host has no `timeout`; --timeout
  bounds the run.
"""
import json, os, re, socket, sys, time

args = sys.argv[1:]
ser_path, qmp_path, outdir, tag = args[:4]
answer = '--answer' in args
def opt(name, default):
    return args[args.index(name) + 1] if name in args else default
end_re = re.compile(opt('--end', r'\[desk\] desktop up').encode())
end_delay = float(opt('--end-delay', '6'))
mark_delay = float(opt('--mark-delay', '1.0'))
MARK_RE = re.compile(rb'\[m3\] MARK (\S+) step')  # whole name only: a chunk may end mid-line
marks_done = 0
mark_due = []
timeout = float(opt('--timeout', '300'))
burst = None
if '--burst' in args:
    i = args.index('--burst')
    burst = (args[i + 1], int(args[i + 2]), float(args[i + 3]))

shots = os.path.abspath(outdir)
serdir = os.path.abspath(os.path.join(outdir, '..', 'serial'))
os.makedirs(shots, exist_ok=True)
os.makedirs(serdir, exist_ok=True)
log = open(os.path.join(serdir, f'{tag}-serial.log'), 'wb')
stamp = open(os.path.join(serdir, f'{tag}-stamped.log'), 'wb') if '--stamp' in args else None
pend = b''

def connect(path):
    t0 = time.time()
    while True:
        try:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.connect(path)
            return s
        except OSError:
            if time.time() - t0 > 30:
                sys.exit(f'cannot connect {path}')
            time.sleep(0.1)

ser = connect(ser_path)
qmp = connect(qmp_path)
qf = qmp.makefile('rw', encoding='utf-8', newline='\n')
def qcmd(obj):
    qf.write(json.dumps(obj) + '\n'); qf.flush()
    while True:
        msg = json.loads(qf.readline())
        if 'return' in msg or 'error' in msg:
            return msg
qf.readline()
qcmd({'execute': 'qmp_capabilities'})

def dump(name):
    path = os.path.join(shots, name)
    r = qcmd({'execute': 'screendump', 'arguments': {'filename': path, 'format': 'png'}})
    if 'error' in r:
        r = qcmd({'execute': 'screendump', 'arguments': {'filename': path}})
    print(f't={time.time()-t0:6.1f}s {name} {json.dumps(r)}', flush=True)

PROMPT = b'*** Press any key to continue ***'
t0 = time.time()
buf = b''
seen = 0
end_at = None
ser.settimeout(0.2)
while True:
    now = time.time()
    if now - t0 > timeout:
        print('timeout', flush=True)
        dump(f'{tag}-timeout.png')
        break
    try:
        chunk = ser.recv(4096)
        if not chunk:
            break
        log.write(chunk); log.flush()
        buf += chunk
        if stamp is not None:
            pend += chunk
            while b'\n' in pend:
                line, pend = pend.split(b'\n', 1)
                stamp.write(b'%10.3f\t' % (time.time() - t0) + line + b'\n')
            stamp.flush()
    except socket.timeout:
        pass
    found = MARK_RE.findall(buf)
    while marks_done < len(found):
        mark_due.append((time.time() + mark_delay, found[marks_done].decode()))
        marks_done += 1
    while mark_due and time.time() >= mark_due[0][0]:
        nm = mark_due.pop(0)[1]
        if nm.startswith('@'):
            dump(f'{tag}-{nm[1:]}.png')
            continue
        dump(f'{tag}-{nm}.png')
        if burst is not None and nm == burst[0]:
            tb = time.time()
            for k in range(burst[1]):
                mark_due.append((tb + (k + 1) * burst[2], f'@{nm}-b{k + 1:02d}'))
            mark_due.sort()
    n = buf.count(PROMPT)
    while seen < n:
        seen += 1
        time.sleep(0.8)
        dump(f'{tag}-crash{seen}.png')
        if answer:
            ser.sendall(b' ')
            print(f't={time.time()-t0:6.1f}s answered prompt {seen}', flush=True)
    if end_at is None and end_re.search(buf):
        end_at = time.time() + end_delay
        print(f't={time.time()-t0:6.1f}s end marker seen', flush=True)
    if end_at is not None and time.time() >= end_at:
        dump(f'{tag}-end.png')
        break
    if len(buf) > 1 << 20:
        # keep the MARK count consistent with the trimmed buffer
        cut = len(buf) - (1 << 16)
        marks_done -= len(MARK_RE.findall(buf[:cut]))
        seen -= buf[:cut].count(PROMPT)
        buf = buf[cut:]
try:
    qcmd({'execute': 'quit'})
except Exception:
    pass
# drain what is left
try:
    ser.settimeout(1.0)
    while True:
        c = ser.recv(4096)
        if not c:
            break
        log.write(c)
except Exception:
    pass
log.close()
if stamp is not None:
    stamp.close()
print(f'prompts seen: {seen}', flush=True)
