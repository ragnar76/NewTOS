#!/usr/bin/env python3
"""qedo-check.py <card.img> -- C:\\APPS\\QED\\NOTE.TXT after the qedo run must be b'ab' + CRLF? + original."""
import struct, subprocess, sys, tempfile, os
img = sys.argv[1]
off = struct.unpack('<I', open(img, 'rb').read(512)[0x1C6:0x1CA])[0] * 512
orig = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'desk/NOTE.TXT'), 'rb').read()
with tempfile.TemporaryDirectory(dir=os.environ.get('TMPDIR')) as t:
    d = os.path.join(t, 'f')
    r = subprocess.run(['mcopy', '-n', '-i', f'{img}@@{off}', '::Apps/QED/NOTE.TXT', d], capture_output=True)
    got = open(d, 'rb').read() if r.returncode == 0 else None
print('original   :', orig)
print('after run  :', got)
ok = got is not None and got != orig and got.startswith(b'ab') and got.endswith(orig)
print('HOSTCHECK:', 'PASS' if ok else 'FAIL')
