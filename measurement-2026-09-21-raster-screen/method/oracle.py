#!/usr/bin/env python3
"""oracle.py -- an independent model of what EmuTOS' rules predict for every
case of tosdump/cases/vdi-raster-screen.tsv, checked against a dump.

The model is written from the EmuTOS source (vdi_raster.c, vdi_fill.c,
vdi_col.c, vdi_control.c), not from either implementation, so a mismatch
between the model and the Falcon dump is a TABLE/MODEL error, and a mismatch
between the Falcon dump and NewTOS is a NewTOS defect.

usage: oracle.py CASES.TSV DUMP.BIN
"""
import sys, struct

REC = 3696; HDR = 32
OFF_CASEID=0; OFF_OPCODE=2; OFF_CONTRL=8; OFF_INTIN=68; OFF_PTSIN=324
OFF_INTOUT=836; OFF_PTSOUT=1092; OFF_SRC=1604; OFF_DST=1620; OFF_A=1636; OFF_B=2660
POISON=0xDEAD
W,H=640,480
CLEAR=0xFFFF

def fill_word(seed, index):
    x = (index * 1103515245 + 12345) & 0xFFFFFFFF
    x ^= (seed * 2654435761) & 0xFFFFFFFF
    x ^= x >> 15
    x = (x * 2246822519) & 0xFFFFFFFF
    x ^= x >> 13
    return x & 0xFFFF

def five(c): return (c*31+500)//1000
def word(r,g,b): return (five(r)<<11)|(five(g)<<6)|five(b)
PAL1=[(1000,1000,1000),(0,0,0),(1000,0,0),(0,1000,0),(0,0,1000),(0,1000,1000),(1000,1000,0),(1000,0,1000),
      (733,733,733),(533,533,533),(667,0,0),(0,667,0),(0,0,667),(0,667,667),(667,667,0),(667,0,667)]
PAL2_0=(1000,1000,1000); PAL2_239=(0,0,0)
def pen(p):
    # validate_color_index (vdi_control.c:224-230): out of range -> 1
    if p < 0 or p >= 256: p = 1
    if p < 16: return word(*PAL1[p])
    if p == 16: return word(*PAL2_0)
    if p == 255: return word(*PAL2_239)
    raise ValueError("pen %d not modelled" % p)

def op(o, s, d):
    o &= 0xF
    return [0, s&d, s&~d&0xFFFF, s, ~s&d&0xFFFF, d, s^d, s|d, ~(s|d)&0xFFFF, ~(s^d)&0xFFFF,
            ~d&0xFFFF, s|(~d&0xFFFF), ~s&0xFFFF, (~s&0xFFFF)|d, ~(s&d)&0xFFFF, 0xFFFF][o]

def parse_table(path):
    cases=[]; pend=[]
    for no,line in enumerate(open(path,encoding='latin-1').read().split('\n'),1):
        t=line.rstrip('\r'); st=t.strip()
        if not st or st.startswith('#') or st.startswith('!'): continue
        col=t.split('\t')
        if st.startswith('+'):
            pend.append(dict(op=int(col[0][1:],0), intin=[int(v,0) for v in col[1].split(',')] if col[1]!='-' else [],
                             ptsin=[int(v,0) for v in col[2].split(',')] if col[2]!='-' else [], src=col[3], dst=col[4]))
            continue
        fill=[int(v,0) for v in col[5].split(',')]
        cases.append(dict(line=no, op=int(col[0],0), intin=[int(v,0) for v in col[1].split(',')] if col[1]!='-' else [],
                          ptsin=[int(v,0) for v in col[2].split(',')], src=col[3], dst=col[4], fill=fill,
                          pre=pend, comment=col[6] if len(col)>6 else ''))
        pend=[]
    return cases

def mfdb(spec):
    if spec=='S': return None
    v=[int(x) for x in spec.split(',')]
    return dict(buf=v[0], w=v[1], h=v[2], wdw=v[3], stand=v[4], planes=v[5])

def sort_rect(p):
    return (min(p[0],p[2]),min(p[1],p[3]),max(p[0],p[2]),max(p[1],p[3]))

class Machine:
    def __init__(s, seed_a, seed_b):
        s.scr={}  # (x,y)->word, default CLEAR
        s.A=[fill_word(seed_a,i) for i in range(512)]
        s.B=[fill_word(seed_b,i) for i in range(512)]
        s.clip=None; s.pen=1
    def get(s, x, y): return s.scr.get((x,y), CLEAR)
    def put(s, x, y, v):
        if s.clip:
            cx1,cy1,cx2,cy2=s.clip
            if not (cx1<=x<=cx2 and cy1<=y<=cy2): return
        if 0<=x<W and 0<=y<H: s.scr[(x,y)]=v
    def buf(s, m): return s.A if m['buf']==0 else s.B
    def mem_get(s, m, x, y): return s.buf(m)[y*m['wdw']*16+x]
    def mem_put(s, m, x, y, v): s.buf(m)[y*m['wdw']*16+x]=v
    def vro(s, mode, pts, src, dst):
        # cpy_raster: mode & ~0x1f invalid -> return; PAT_FLAG on 16-bit -> no arm -> nothing
        if mode & ~0x1F: return
        sm=mfdb(src); dm=mfdb(dst)
        s_planes = 16 if sm is None else sm['planes']
        d_planes = 16 if dm is None else dm['planes']
        if d_planes > 16: return
        if s_planes != d_planes: return
        if d_planes > 8 and mode & 0x10: return   # vro_cpyfm16 re-reads INTIN[0], no arm
        sx1,sy1,sx2,sy2=sort_rect(pts[0:4]); dx1,dy1,dx2,dy2=sort_rect(pts[4:8])
        bw=sx2-sx1+1; bh=sy2-sy1+1
        # clipping only when dst is the screen and clip on (do_clip)
        if dm is None and s.clip:
            cx1,cy1,cx2,cy2=s.clip
            if dx1<cx1: sx1-= dx1-cx1; dx1=cx1
            dx2=sx2-sx1+dx1
            if dx2>cx2: dx2=cx2
            if dx2-dx1<0: return
            bw=dx2-dx1+1
            if dy1<cy1: sy1-= dy1-cy1; dy1=cy1
            dy2=sy2-sy1+dy1
            if dy2>cy2: dy2=cy2
            if dy2-dy1<0: return
            bh=dy2-dy1+1
        # direction: compare start addresses (vro_cpyfm16 :993). Same form only
        # matters when src and dst are the same memory; here model by linear index
        def lin_s(): return (sy1*(640 if sm is None else sm['wdw']*16)+sx1)
        def lin_d(): return (dy1*(640 if dm is None else dm['wdw']*16)+dx1)
        same = (sm is None and dm is None) or (sm is not None and dm is not None and sm['buf']==dm['buf'])
        reverse = same and lin_s() < lin_d()
        if not same:
            # different memories: address order is the linker's; for A->B, A precedes B
            # (struct order) so src<dst -> reverse; screen vs memory: unknowable but
            # irrelevant because no overlap.
            reverse = False
        rows=range(bh-1,-1,-1) if reverse else range(bh)
        cols=range(bw-1,-1,-1) if reverse else range(bw)
        getS=(lambda x,y: s.get(x,y)) if sm is None else (lambda x,y: s.mem_get(sm,x,y))
        getD=(lambda x,y: s.get(x,y)) if dm is None else (lambda x,y: s.mem_get(dm,x,y))
        putD=(lambda x,y,v: s.scr.__setitem__((x,y),v)) if dm is None else (lambda x,y,v: s.mem_put(dm,x,y,v))
        for r in rows:
            for c in cols:
                putD(dx1+c, dy1+r, op(mode, getS(sx1+c,sy1+r), getD(dx1+c,dy1+r)))
    def vrt(s, intin, pts, src, dst):
        mode=intin[0]
        if mode & ~0x1F: return
        sm=mfdb(src); dm=mfdb(dst)
        if sm is None or sm['planes']!=1: return
        d_planes = 16 if dm is None else dm['planes']
        if d_planes > 16: return
        fg=pen(intin[1]); bg=pen(intin[2])
        if d_planes > 8:
            # vrt_cpyfm16 re-reads INTIN[0]; only exactly 1..4 match
            if mode not in (1,2,3,4): return
        sx1,sy1,sx2,sy2=sort_rect(pts[0:4]); dx1,dy1,dx2,dy2=sort_rect(pts[4:8])
        bw=sx2-sx1+1; bh=sy2-sy1+1
        if dm is None and s.clip:
            cx1,cy1,cx2,cy2=s.clip
            if dx1<cx1: sx1-= dx1-cx1; dx1=cx1
            dx2=sx2-sx1+dx1
            if dx2>cx2: dx2=cx2
            if dx2-dx1<0: return
            bw=dx2-dx1+1
            if dy1<cy1: sy1-= dy1-cy1; dy1=cy1
            dy2=sy2-sy1+dy1
            if dy2>cy2: dy2=cy2
            if dy2-dy1<0: return
            bh=dy2-dy1+1
        getD=(lambda x,y: s.get(x,y)) if dm is None else (lambda x,y: s.mem_get(dm,x,y))
        putD=(lambda x,y,v: s.scr.__setitem__((x,y),v)) if dm is None else (lambda x,y,v: s.mem_put(dm,x,y,v))
        for r in range(bh):
            for c in range(bw):
                x=sx1+c; y=sy1+r
                wv=s.buf(sm)[y*sm['wdw']+x//16]
                bit=(wv>>(15-(x&15)))&1
                X=dx1+c; Y=dy1+r
                if mode==1: putD(X,Y, fg if bit else bg)
                elif mode==2:
                    if bit: putD(X,Y,fg)
                elif mode==3:
                    if bit: putD(X,Y, ~getD(X,Y)&0xFFFF)
                elif mode==4:
                    if not bit: putD(X,Y,bg)
    def prelude(s, p):
        if p['op']==25: s.pen=p['intin'][0]
        elif p['op']==114:
            x1,y1,x2,y2=sort_rect(p['ptsin'])
            for y in range(y1,y2+1):
                for x in range(x1,x2+1): s.put(x,y,pen(s.pen))
        elif p['op']==129:
            s.clip = tuple(sort_rect(p['ptsin'])) if p['intin'][0] else None
        elif p['op']==109: s.vro(p['intin'][0] if p['intin'] else POISON, p['ptsin'], p['src'], p['dst'])
        elif p['op']==121: s.vrt(p['intin'], p['ptsin'], p['src'], p['dst'])

def expected(case):
    m=Machine(case['fill'][0], case['fill'][1])
    for p in case['pre']: m.prelude(p)
    if case['op']==109: m.vro(case['intin'][0] if case['intin'] else POISON, case['ptsin'], case['src'], case['dst'])
    elif case['op']==121: m.vrt(case['intin'], case['ptsin'], case['src'], case['dst'])
    if len(case['fill'])==6:
        rx,ry,rw,rh=case['fill'][2:6]
        for r in range(rh):
            for c in range(rw):
                m.B[r*32+c]=m.get(rx+c, ry+r)
    return m.A, m.B

def be16(b,o): return (b[o]<<8)|b[o+1]

def main():
    cases=parse_table(sys.argv[1]); dump=open(sys.argv[2],'rb').read()
    n=be16(dump,10)
    assert n==len(cases), (n,len(cases))
    bad=0
    for i,case in enumerate(cases):
        rec=dump[HDR+i*REC:HDR+(i+1)*REC]
        assert be16(rec,OFF_CASEID)==case['line']
        A=[be16(rec,OFF_A+2*k) for k in range(512)]
        B=[be16(rec,OFF_B+2*k) for k in range(512)]
        eA,eB=expected(case)
        da=[k for k in range(512) if A[k]!=eA[k]]
        db=[k for k in range(512) if B[k]!=eB[k]]
        # array hygiene: intout/ptsout poisoned, intin/ptsin echoed
        hy=[]
        for k in range(128):
            if be16(rec,OFF_INTOUT+2*k)!=POISON: hy.append('intout[%d]'%k)
        for k in range(256):
            if be16(rec,OFF_PTSOUT+2*k)!=POISON: hy.append('ptsout[%d]'%k)
        for k,v in enumerate(case['intin']):
            if be16(rec,OFF_INTIN+2*k)!=v&0xFFFF: hy.append('intin[%d]'%k)
        for k,v in enumerate(case['ptsin']):
            if be16(rec,OFF_PTSIN+2*k)!=v&0xFFFF: hy.append('ptsin[%d]'%k)
        # a screen MFDB is recorded as eight zeros
        for name,off,spec in (('src',OFF_SRC,case['src']),('dst',OFF_DST,case['dst'])):
            if spec=='S' and any(be16(rec,off+2*k) for k in range(8)): hy.append(name+'_mfdb')
        tag='ok' if not (da or db or hy) else 'MISMATCH'
        if tag!='ok': bad+=1
        print('case %3d %s  A:%3d B:%3d %s  %s' % (case['line'], tag, len(da), len(db), ','.join(hy[:4]), case['comment'][:60]))
        if db[:6]:
            for k in db[:6]: print('      B[%3d] (r%2d c%2d) dump=%04X model=%04X' % (k,k//32,k%32,B[k],eB[k]))
    print('%d of %d cases mismatch the model' % (bad,len(cases)))
if __name__=='__main__': main()
