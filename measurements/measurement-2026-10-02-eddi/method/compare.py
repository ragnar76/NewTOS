#!/usr/bin/env python3
"""compare.py <enhancer SCRNINFO.TXT> <newtos SCRNINFO.TXT>

Side by side: section 3b (vq_scrninfo) of two SCRNINFO.TXT reports -- the
273 work_out words and CONTRL[2]/[4] after the call -- plus the cookie line
and the dispatcher line. Prints every word, marks each difference with '<>'."""
import re, sys

def parse(path):
    txt = open(path, encoding='latin-1').read()
    w = {}
    for m in re.finditer(r'^work_out\[\s*(\d+)\]((?:\s+-?\d+/[0-9A-F]{4})+)', txt, re.M):
        base = int(m.group(1))
        for k, cell in enumerate(re.findall(r'(-?\d+)/([0-9A-F]{4})', m.group(2))):
            w[base + k] = int(cell[1], 16)
    contrl = re.search(r'^CONTRL after: (.*)$', txt, re.M).group(1)
    eddi = re.search(r'^EdDI: (.*)$', txt, re.M).group(1)
    disp = re.search(r'== 2\. EdDI DISPATCHER, FUNCTION 0 ==\n(.*)\n', txt).group(1)
    rest = [l for l in txt.splitlines() if l.startswith(('INTOUT[273', 'PTSOUT', 'summary:'))]
    return w, contrl, eddi, disp, rest

a, b = parse(sys.argv[1]), parse(sys.argv[2])
print('A = %s' % sys.argv[1])
print('B = %s' % sys.argv[2])
print()
for name, i in (('cookie', 2), ('dispatcher(0)', 3), ('CONTRL after', 1)):
    print('%-14s A: %s' % (name, a[i]))
    print('%-14s B: %s' % ('', b[i]))
for la, lb in zip(a[4], b[4]):
    print('A: %s\nB: %s' % (la, lb))
print()
print('work_out  A(Enhancer)  B(NewTOS)')
nd = 0
for i in range(273):
    x, y = a[0].get(i), b[0].get(i)
    mark = '' if x == y else '   <> differs'
    nd += (x != y)
    print('[%3d]     %04X         %04X%s' % (i, x, y, mark))
print()
print('words compared: 273, differing: %d' % nd)
