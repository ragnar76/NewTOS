#!/usr/bin/env python3
"""drive-ev.py SERIAL_SOCK QMP_SOCK OUTDIR TAG --event REGEX --shots S:NAME[,S:NAME...] [--tail S] [--timeout S]
           [--prompt-shot] [--until REGEX]

DESK43 U9-1/U9-2 driver (pattern of drive-u6a.py: serial from a QEMU `-serial unix:...,server=on,wait=on`
socket, verbatim to OUTDIR/../serial/TAG-serial.log; QMP screendumps; the run bounded here, the host has no
`timeout`). The screendumps are timed from an EVENT on the serial line, not from the QEMU start -- the EL0
desktop comes up at another moment than the EL1 one, and the views must be taken in the same phase:

* first line matching --event          -> clock T0 (wall time of the host when the line arrived);
* every --shots S:NAME                  -> TAG-NAME.png at T0 + S seconds;
* "*** Press any key to continue ***"   -> with --prompt-shot: 0.8 s later TAG-promptN.png;
* after the last shot (or the first --until match), --tail seconds more of serial, then quit.
"""
import json, os, re, socket, sys, time

args = sys.argv[1:]
ser_path, qmp_path, outdir, tag = args[:4]
def opt(name, default):
    return args[args.index(name) + 1] if name in args else default
event = re.compile(opt('--event', r'\[desk\] desktop up').encode())
shots = []
for spec in filter(None, opt('--shots', '').split(',')):
    s, n = spec.split(':', 1)
    shots.append((float(s), n))
shots.sort()
tail = float(opt('--tail', '3'))
timeout = float(opt('--timeout', '300'))
until = opt('--until', None)
until = re.compile(until.encode()) if until else None
prompt_shot = '--prompt-shot' in args

outdir = os.path.abspath(outdir)
serdir = os.path.abspath(os.path.join(outdir, '..', 'serial'))
os.makedirs(outdir, exist_ok=True)
os.makedirs(serdir, exist_ok=True)
log = open(os.path.join(serdir, f'{tag}-serial.log'), 'wb')

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

start = time.time()
def dump(name):
    path = os.path.join(outdir, name)
    r = qcmd({'execute': 'screendump', 'arguments': {'filename': path, 'format': 'png'}})
    if 'error' in r:
        r = qcmd({'execute': 'screendump', 'arguments': {'filename': path}})
    print(f't={time.time()-start:6.1f}s {name} {json.dumps(r)}', flush=True)

PROMPT = b'*** Press any key to continue ***'
buf = b''
t0 = None
nshot = 0
seen_prompts = 0
stop_at = None
ser.settimeout(0.05)
while True:
    now = time.time()
    if now - start > timeout:
        print('timeout', flush=True)
        dump(f'{tag}-timeout.png')
        break
    try:
        chunk = ser.recv(4096)
        if not chunk:
            break
        log.write(chunk); log.flush()
        buf += chunk
    except socket.timeout:
        pass
    if t0 is None and event.search(buf):
        t0 = time.time()
        print(f't={t0-start:6.1f}s event seen', flush=True)
    if t0 is not None:
        while nshot < len(shots) and time.time() >= t0 + shots[nshot][0]:
            dump(f'{tag}-{shots[nshot][1]}.png')
            nshot += 1
        if stop_at is None and nshot == len(shots) and (until is None or until.search(buf)):
            stop_at = time.time() + tail
    if prompt_shot:
        n = buf.count(PROMPT)
        while seen_prompts < n:
            seen_prompts += 1
            time.sleep(0.8)
            dump(f'{tag}-prompt{seen_prompts}.png')
    if stop_at is not None and time.time() >= stop_at:
        break
try:
    qcmd({'execute': 'quit'})
except Exception:
    pass
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
print(f'{tag}: event {"seen" if t0 else "NOT seen"}, shots {nshot}/{len(shots)}, prompts {seen_prompts}', flush=True)
