#!/usr/bin/env python3
"""Dump a classic (big-endian) GEM RSC tree with rsrc_obfix'd pixel coords (8x16 font)."""
import struct, sys
d = open(sys.argv[1], 'rb').read()
tree_no = int(sys.argv[2])
WC, HC = 8, 16
h = struct.unpack('>18H', d[:36])
(vrsn, ob_off, ted_off, ib_off, bb_off, frstr_off, str_off, imdata_off, frimg_off,
 trindex_off, nobs, ntree, nted, nib, nbb, nstring, nimages, rssize) = h
tr = struct.unpack('>%dI' % ntree, d[trindex_off:trindex_off + 4 * ntree])
base = tr[tree_no]
def fix(v):
    return (v & 0xff), (v >> 8) & 0xff
def obj(i):
    o = base + 24 * i
    nxt, head, tail, typ, flags, state, spec, x, y, w, h_ = struct.unpack('>hhhHHHIHHHH', d[o:o+24])
    xc, xp = fix(x); yc, yp = fix(y); wc, wp = fix(w); hc, hp = fix(h_)
    return nxt, head, tail, typ, flags, state, spec, xc*WC+xp, yc*HC+yp, wc*WC+wp, hc*HC+hp
def text(spec, typ):
    t = typ & 0xff
    try:
        if t in (28, 26, 32):  # G_STRING, G_BUTTON, G_TITLE
            e = d.index(b'\0', spec); return d[spec:e].decode('latin1')
        if t in (21, 22, 29, 30):  # TEXT, BOXTEXT, FTEXT, FBOXTEXT -> TEDINFO
            te = struct.unpack('>III', d[spec:spec+12])[0]
            e = d.index(b'\0', te); return d[te:e].decode('latin1')
    except Exception:
        return '?'
    return ''
# walk with absolute coords
def walk(i, ax, ay, depth):
    nxt, head, tail, typ, flags, state, spec, x, y, w, h_ = obj(i)
    X, Y = ax + x, ay + y
    print("%s#%d type=%d flags=%04x state=%04x rel=(%d,%d,%d,%d) abs=(%d,%d)-(%d,%d) %s" % (
        '  '*depth, i, typ & 0xff, flags, state, x, y, w, h_, X, Y, X+w-1, Y+h_-1, repr(text(spec, typ))))
    c = head
    while c != -1 and c != i:
        walk(c, X, Y, depth+1)
        cn = obj(c)[0]
        if cn == i: break
        c = cn
walk(0, 0, 0, 0)
