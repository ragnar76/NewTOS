#!/usr/bin/env python3
"""card-check.py <kind> <card.img> -- host-side check of a GUI write run's
card (prep-card.sh kinds). Files are pulled with mtools at C:'s partition
offset and compared byte for byte with their sources in method/desk/ or the
gen-pattern.py model; names that must be gone are checked with mdir.
  desk  C:\\SUB3A\\{INSIDE.TXT, BIG.DAT, NOTE.TXT} = the originals (folder copy
        of C:\\D3A\\SUB3A after copy BIG.DAT + move NOTE.TXT into it);
        C:\\D3A\\ empty (BIG.DAT deleted, NOTE.TXT moved, SUB3A deleted)
  tera  C:\\ZVIEW\\DUMPVDI.NAP = C:\\DUMPVDI.NAP (file copy) and every file of
        C:\\COREMARK\\ = its copy in C:\\APPS\\COREMARK\\ (folder copy)
  qed   QEDT.TXT (wherever the file selector put it) holds what QED had after
        the second save: b'fs08\r\nab\r\n' (unshifted keys; the second save
        rewrote the file the first one created)
Prints one line per item and "HOSTCHECK: PASS|FAIL"."""
import os, struct, subprocess, sys, tempfile
kind, img = sys.argv[1], sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
off = struct.unpack('<I', open(img, 'rb').read(512)[0x1C6:0x1CA])[0] * 512
I = f'{img}@@{off}'
def pat(seed, n):
    return bytes((((i * 7) + (i // 251)) & 0xFF) ^ seed for i in range(n))
src = {'INSIDE.TXT': open(os.path.join(HERE, 'desk/INSIDE.TXT'), 'rb').read(),
       'NOTE.TXT': open(os.path.join(HERE, 'desk/NOTE.TXT'), 'rb').read(),
       'BIG.DAT': pat(0x77, 300003)}
bad = 0
def get(path):
    with tempfile.TemporaryDirectory(dir=os.environ.get('TMPDIR')) as t:
        dst = os.path.join(t, 'f')
        r = subprocess.run(['mcopy', '-n', '-i', I, '::' + path, dst], capture_output=True)
        return open(dst, 'rb').read() if r.returncode == 0 else None
def same(path, want):
    global bad
    got = get(path)
    if got == want:
        print(f'ok      {path} {len(got)} B')
    else:
        bad += 1
        print(f'DIFF    {path}: ' + ('missing' if got is None else f'{len(got)} B, want {len(want)}'))
def listing(path):
    r = subprocess.run(['mdir', '-b', '-i', I, '::' + path], capture_output=True, text=True)
    return [l for l in r.stdout.split('\n') if l.strip()] if r.returncode == 0 else None
if kind == 'desk':
    for n in ('INSIDE.TXT', 'BIG.DAT', 'NOTE.TXT'):
        same(f'SUB3A/{n}', src[n])
    left = listing('D3A')
    left = [l for l in (left or []) if not l.endswith('/.') and not l.endswith('/..')]
    if left:
        bad += 1; print(f'PRESENT in D3A: {left}')
    else:
        print('ok      D3A empty')
elif kind == 'tera':
    same('ZVIEW/DUMPVDI.NAP', get('DUMPVDI.NAP'))
    r = subprocess.run(['mdir', '-/', '-b', '-i', I, '::COREMARK'], capture_output=True, text=True)
    srcs = [l.strip()[3:] for l in r.stdout.split('\n') if l.strip() and not l.strip().endswith('/')]
    if not srcs:
        bad += 1; print('no files under C:\\COREMARK')
    for p in srcs:
        same('APPS/' + p, get(p))
elif kind == 'qed':
    r = subprocess.run(['mdir', '-/', '-b', '-i', I, '::'], capture_output=True, text=True)
    hits = [l.strip() for l in r.stdout.split('\n') if l.strip().upper().endswith('/QEDT.TXT')]
    if len(hits) != 1:
        bad += 1; print(f'QEDT.TXT found {len(hits)} times: {hits}')
    else:
        got = get(hits[0][3:] if hits[0].startswith('::/') else hits[0])
        print(f'file    {hits[0]}: {got!r}')
        if got != b'fs08\r\nab\r\n':
            bad += 1; print("DIFF    want b'fs08\\r\\nab\\r\\n'")
        else:
            print('ok      both saves on the card')
print('HOSTCHECK: ' + ('PASS' if bad == 0 else f'FAIL ({bad})'))
