#!/usr/bin/env python3
"""layout-model.py DECK SPACING [SPACING...] -- host model of PRESENT.NAP's
non-centred slide layout at 1920x1080 (8x16 system font), transcribed from
sdk/apps/present/present.c render_slide()/items_block()/text_block()/wrap_line()
with item spacing = (ch*k*SPACING)/12. Per slide: body scale B chosen, body
height at B=3 vs the room (cy1-cy0), slack, and per item the wrapped lines.
Only an aid to read the screendumps -- the dumps are the evidence."""
import sys
cw, ch, SW, SH = 8, 16, 1920, 1080
def clampi(v, lo, hi): return lo if v < lo else hi if v > hi else v
def wrap_line(t, maxc):
    n = len(t); maxc = max(maxc, 1)
    if n <= maxc: return n, n
    i = maxc
    while i > 0 and t[i] != ' ': i -= 1
    if i == 0: return maxc, maxc
    e = i
    while e > 0 and t[e-1] == ' ': e -= 1
    a = i
    while a < n and t[a] == ' ': a += 1
    return e, a
def lines(t, k, x0, x1):
    maxc = (x1 - x0) // (cw * k); out = []
    while t:
        ln, adv = wrap_line(t, maxc); out.append(t[:ln]); t = t[adv:]
        if adv == 0: break
    return out
def lineh(k): return ch * k + (ch * k) // 8
def items_block(items, B, x0, x1, sp):
    S = B - 1 if B >= 3 else B; y = 0; det = []
    for kind, text in items:
        k = S if kind == 'sub' else B
        if kind == 'gap': y += (ch * B) // 2; continue
        if kind == 'bullet': tx = x0 + cw * B * 2
        elif kind == 'sub': tx = x0 + cw * B * 2 + cw * S * 2
        else: tx = x0
        L = lines(text, k, tx, x1); y += len(L) * lineh(k); det.append((kind, L, (x1 - tx) // (cw * k)))
        y += (ch * k * sp) // 12
    return y, det
def parse(path):
    slides = []; cur = None
    for raw in open(path, encoding='utf-8'):
        line = raw.rstrip('\r\n').rstrip()
        s = line.lstrip(' \t'); ind = len(line) - len(s)
        if s.startswith('# ') and ind == 0:
            cur = {'title': s[2:].strip(), 'image': None, 'center': False, 'items': []}; slides.append(cur); continue
        if s.startswith(';'): continue
        if s.startswith('@') and ind == 0:
            if cur is not None:
                if s.startswith('@image'): cur['image'] = s[6:].strip()
                if s.startswith('@center'): cur['center'] = True
            continue
        if cur is None: continue
        if not s:
            if cur['items'] and cur['items'][-1][0] != 'gap': cur['items'].append(('gap', None))
            continue
        if s[0] in '-*' and (len(s) == 1 or s[1] == ' '):
            cur['items'].append(('sub' if ind >= 2 else 'bullet', s[1:].strip())); continue
        cur['items'].append(('text', s))
    for c in slides:
        while c['items'] and c['items'][-1][0] == 'gap': c['items'].pop()
    return slides
deck = sys.argv[1]
for sp in map(int, sys.argv[2:]):
    print(f"===== spacing {sp}")
    for i, sl in enumerate(parse(deck), 1):
        if sl['center'] or not sl['items']:
            print(f"S{i:02d} {sl['title']!r}: {'centred' if sl['center'] else 'picture only'} (not modelled)"); continue
        mx, my = SW // 20, SH // 18; x0, x1 = mx, SW - mx; tw = x1 - x0
        fs = clampi((SH * 3 // 2 + 540) // 1080, 1, 4); bt = max(SH // 360, 2)
        fy = SH - my // 2 - ch * fs; cy1 = fy - SH // 60
        T = clampi((SH * 4 + 540) // 1080, 1, 8)
        y = my + len(lines(sl['title'], T, x0, x1)) * lineh(T) + (ch * T) // 4 + bt
        cy0 = y + SH // 25; room = cy1 - cy0
        txt_x1 = x0 + tw * 56 // 100 if sl['image'] else x1
        B = 3
        while True:
            h, det = items_block(sl['items'], B, x0, txt_x1, sp)
            if B == 1 or h <= room: break
            B = {8: 6, 6: 4}.get(B, B - 1)
        h3, det3 = items_block(sl['items'], 3, x0, txt_x1, sp)
        print(f"S{i:02d} {sl['title']!r}: scale x{B}  body@x3 {h3}px / room {room}px  slack {room - h3:+d}px")
        for kind, L, maxc in det3:
            if len(L) > 1:
                print(f"     wraps ({kind}, {maxc} cols): " + " | ".join(L))
