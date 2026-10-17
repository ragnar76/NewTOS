#!/usr/bin/env python3
"""gen_extra.py <scripts-dir> -- writes kbn.script (negative reset keys on the desktop, then the positive control Ctrl+Alt+Del)
and qt.script (QED typing with both Shift keys) as IKBD byte scripts for tools/inject-arm.py (<ticks> <byte> ...).
Atari scancodes: Ctrl 1D, LShift 2A, RShift 36, Alt 38, Delete 53, Left 4B, h 23, e 12, l 26, o 18, space 39, w 11, r 13, d 20, q 10, x 2D, a 1E, b 30, c 2E, Return 1C."""
import sys
d = sys.argv[1]
def emit(path, seqs, t0, step, head):
    t = t0; out = []
    for s in seqs:
        for b in s:
            out.append("%d 0x%02X" % (t, b)); t += step
        t += step * 3
    open(path, "w").write(head + "\n".join(out) + "\n")
DEL, DELB = 0x53, 0xD3
neg = [
    [DEL, DELB],                                          # Delete alone
    [0x1D, DEL, DELB, 0x9D],                              # Ctrl+Del
    [0x38, DEL, DELB, 0xB8],                              # Alt+Del
    [0x2A, DEL, DELB, 0xAA],                              # LShift+Del
    [0x36, DEL, DELB, 0xB6],                              # RShift+Del
    [0x1D, 0x36, DEL, DELB, 0xB6, 0x9D],                  # Ctrl+RShift+Del (no Alt)
    [0x38, 0x36, DEL, DELB, 0xB6, 0xB8],                  # Alt+RShift+Del (no Ctrl)
    [0x1D, 0x2A, 0x38, DEL, DELB, 0xB8, 0xAA, 0x9D],      # Ctrl+LShift+Alt+Del
    [0x1D, 0x2A, 0x36, 0x38, DEL, DELB, 0xB8, 0xB6, 0xAA, 0x9D],  # Ctrl+LShift+RShift+Alt+Del (LShift excludes)
    [0x23, 0xA3, 0x12, 0x92, 0x26, 0xA6],                 # plain typing h e l
    [0x1D, 0x38, DEL, DELB, 0xB8, 0x9D],                  # POSITIVE CONTROL: Ctrl+Alt+Del -> warm reset
]
emit(d + "/kbn.script", neg, 400, 8, "# RST01-test: negative reset keys, armed desktop; the LAST sequence is the positive control (only it may reset)\n")
LS, LSB, RS, RSB = 0x2A, 0xAA, 0x36, 0xB6
def ch(code, sh=None):
    mk, bk = code, code | 0x80
    if sh == 'L': return [LS, mk, bk, LSB]
    if sh == 'R': return [RS, mk, bk, RSB]
    if sh == 'B': return [LS, RS, mk, bk, RSB, LSB]
    return [mk, bk]
typ = [[0x1D, 0x31, 0xB1, 0x9D], [0x39, 0xB9]]  # Ctrl+N (File > New) opens an empty window; then a settle gap
for c in [ch(0x23,'L'), ch(0x12), ch(0x26,'R'), ch(0x26), ch(0x18), ch(0x39), ch(0x11,'L'), ch(0x18), ch(0x13,'R'), ch(0x26), ch(0x20),
          ch(0x10,'B'),                                   # both Shifts: Q
          ch(0x2D),                                       # x
          ch(0x4B), ch(DEL)]:                             # Left, Delete alone -> removes x
    typ.append(c)
emit(d + "/qt.script", typ, 2500, 8, "# RST01-test: QED typing 'HeLlo WoRldQ' (H=LShift, L=RShift, W=LShift, R=RShift, Q=both) then x, Left, Delete (alone)\n")
