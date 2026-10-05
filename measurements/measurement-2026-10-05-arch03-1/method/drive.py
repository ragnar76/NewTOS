#!/usr/bin/env python3
"""drive.py -- serial + QMP driver for the ARCH03 0c crash runs.

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
timeout = float(opt('--timeout', '300'))

shots = os.path.abspath(outdir)
serdir = os.path.abspath(os.path.join(outdir, '..', 'serial'))
os.makedirs(shots, exist_ok=True)
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
    except socket.timeout:
        pass
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
        buf = buf[-(1 << 16):]
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
print(f'prompts seen: {seen}', flush=True)
