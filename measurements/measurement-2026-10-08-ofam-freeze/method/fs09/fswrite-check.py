#!/usr/bin/env python3
"""fswrite-check.py <card.img> -- host-side check of FSWRITE.NCP's result.

Extracts D:\\FSW\\, D:\\FSW\\SUB\\ and C:\\FSWC\\ with mtools and compares
every byte against the same content model as method/fswrite.c (written
independently here, so a read/write bug that is symmetric inside the kernel
still shows). Also checks that the deleted/moved names and D:\\FSW\\TMP are
gone. Prints one line per file and "HOSTCHECK: PASS|FAIL".
"""
import struct, subprocess, sys, tempfile, os

def pat(seed, i):
    return (((i * 7) + (i // 251)) & 0xFF) ^ seed

def run(seed, n):
    return bytes(pat(seed, i) for i in range(n))

def model(size, seed, patches):
    b = bytearray(run(seed, size))
    for off, ln, s in patches:
        if off + ln > len(b):
            b.extend(b'\0' * (off + ln - len(b)))
        b[off:off + ln] = run(s, ln)
    return bytes(b)

FILES = {  # path on card -> (size, seed, patches)
    'D/FSW/A00.DAT': (0, 0x01, []),
    'D/FSW/A01.DAT': (1, 0x02, []),
    'D/FSW/A03.DAT': (512, 0x04, []),
    'D/FSW/A04.DAT': (513, 0x05, []),
    'D/FSW/SUB/MOVED05.DAT': (4095, 0x06, []),
    'D/FSW/A06.DAT': (8192, 0x07, []),
    'D/FSW/A08.DAT': (65539, 0x09, []),
    'D/FSW/A09.DAT': (65539, 0x0A, []),
    'D/FSW/A10.DAT': (200003, 0x0B, []),
    'D/FSW/A11.DAT': (1048653, 0x0C, [(8192 - 100, 300, 0x40), (3 * 8192 + 511, 20000, 0x41)]),
    'D/FSW/A12.DAT': (1048653, 0x0D, []),
    'D/FSW/A13.DAT': (3000000, 0x0E, [(3000000, 1000, 0x42), (0, 10, 0x43)]),
    'D/FSW/SUB/COPY13.DAT': (3000000, 0x0E, [(3000000, 1000, 0x42), (0, 10, 0x43)]),
    'C/FSWC/B00.DAT': (100003, 0x21, []),
    'C/FSWC/B01.DAT': (100003, 0x21, []),
}
GONE = ['D/FSW/A02.DAT', 'D/FSW/A05.DAT', 'D/FSW/A07.DAT', 'D/FSW/TMP']

img = sys.argv[1]
mbr = open(img, 'rb').read(512)
off = {'C': struct.unpack('<I', mbr[0x1C6:0x1CA])[0] * 512,
       'D': struct.unpack('<I', mbr[0x1D6:0x1DA])[0] * 512}
bad = 0
with tempfile.TemporaryDirectory(dir=os.environ.get('TMPDIR')) as t:
    for path, (size, seed, patches) in FILES.items():
        drv, rest = path.split('/', 1)
        dst = os.path.join(t, path.replace('/', '_'))
        r = subprocess.run(['mcopy', '-n', '-i', f'{img}@@{off[drv]}', '::' + rest, dst],
                           capture_output=True)
        if r.returncode != 0:
            print(f'MISSING {path}: {r.stderr.decode().strip()}'); bad += 1; continue
        got = open(dst, 'rb').read()
        want = model(size, seed, patches)
        if got == want:
            print(f'ok      {path} {len(got)} B')
        else:
            first = next((i for i in range(min(len(got), len(want))) if got[i] != want[i]), None)
            print(f'DIFF    {path}: len {len(got)} want {len(want)}, first diff at {first}'); bad += 1
    for path in GONE:
        drv, rest = path.split('/', 1)
        r = subprocess.run(['mdir', '-b', '-i', f'{img}@@{off[drv]}', '::' + rest], capture_output=True)
        if r.returncode == 0 and r.stdout.strip():
            print(f'PRESENT {path} (should be gone)'); bad += 1
        else:
            print(f'gone    {path}')
print('HOSTCHECK: ' + ('PASS' if bad == 0 else f'FAIL ({bad})'))
