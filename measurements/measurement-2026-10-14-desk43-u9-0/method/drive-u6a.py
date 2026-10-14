#!/usr/bin/env python3
"""drive-u6a.py -- serial + QMP driver for the DESK43 U6a HELLOGEM runs.

usage: drive-u6a.py SERIAL_SOCK QMP_SOCK OUTDIR TAG [--app-delay S]
                    [--end-delay S] [--timeout S]

Pattern of measurement-2026-10-13-mem05-acceptance/method/drive.py (serial
read from a QEMU `-serial unix:...,server=on,wait=on` socket, QMP screendumps,
the run bounded here because the host has no `timeout`), with the U6a events:

* serial -> OUTDIR/../serial/TAG-serial.log, verbatim;
* first "[launch] EL0"           -> after APP_DELAY s (default 5): TAG-app.png
                                    (the program's window, before the key the
                                    injector feeds 8 s after the start);
* "*** Press any key"            -> 0.8 s later TAG-crashN.png (an exception
                                    report: a failure of the run), end 3 s later;
* "sh_ldapp: ... exited" and then
  "[desk] desktop up"            -> after END_DELAY s (default 6): TAG-end.png,
                                    quit.
"""
import json, os, re, socket, sys, time

args = sys.argv[1:]
ser_path, qmp_path, outdir, tag = args[:4]
def opt(name, default):
    return args[args.index(name) + 1] if name in args else default
app_delay = float(opt('--app-delay', '5'))
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

t0 = time.time()
def dump(name):
    path = os.path.join(shots, name)
    r = qcmd({'execute': 'screendump', 'arguments': {'filename': path, 'format': 'png'}})
    if 'error' in r:
        r = qcmd({'execute': 'screendump', 'arguments': {'filename': path}})
    print(f't={time.time()-t0:6.1f}s {name} {json.dumps(r)}', flush=True)

PROMPT = b'*** Press any key to continue ***'
LAUNCH = re.compile(rb'\[launch\] EL0')
EXITED = re.compile(rb'sh_ldapp: .* exited with rc=')
DESKUP = re.compile(rb'\[desk\] desktop up')
buf = b''
seen = 0
app_at = None
app_done = False
exit_pos = None
end_at = None
stop_at = None
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
        stop_at = time.time() + 3
    if app_at is None and LAUNCH.search(buf):
        app_at = time.time() + app_delay
        print(f't={time.time()-t0:6.1f}s launch seen', flush=True)
    if app_at is not None and not app_done and time.time() >= app_at:
        dump(f'{tag}-app.png')
        app_done = True
    if exit_pos is None:
        m = EXITED.search(buf)
        if m:
            exit_pos = m.end()
            print(f't={time.time()-t0:6.1f}s program exited', flush=True)
    if exit_pos is not None and end_at is None and DESKUP.search(buf, exit_pos):
        end_at = time.time() + end_delay
        print(f't={time.time()-t0:6.1f}s desktop up after the program', flush=True)
    if end_at is not None and time.time() >= end_at:
        if app_at is not None and not app_done:
            dump(f'{tag}-app.png')
        dump(f'{tag}-end.png')
        break
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
print(f'prompts seen: {seen}', flush=True)
