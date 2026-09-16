import sys, zlib, struct
def read_png(p):
    d=open(p,'rb').read()
    assert d[:8]==b'\x89PNG\r\n\x1a\n'
    i=8; w=h=bd=ct=None; idat=b''
    while i<len(d):
        ln=struct.unpack('>I',d[i:i+4])[0]; typ=d[i+4:i+8]; data=d[i+8:i+8+ln]; i+=12+ln
        if typ==b'IHDR': w,h,bd,ct=struct.unpack('>IIBB',data[:10])
        elif typ==b'IDAT': idat+=data
        elif typ==b'IEND': break
    raw=zlib.decompress(idat)
    bpp={0:1,2:3,3:1,4:2,6:4}[ct]*(bd//8)
    stride=w*bpp; out=bytearray(); prev=bytearray(stride); pos=0
    for _ in range(h):
        f=raw[pos]; pos+=1; line=bytearray(raw[pos:pos+stride]); pos+=stride
        if f==1:
            for x in range(bpp,stride): line[x]=(line[x]+line[x-bpp])&255
        elif f==2:
            for x in range(stride): line[x]=(line[x]+prev[x])&255
        elif f==3:
            for x in range(stride):
                a=line[x-bpp] if x>=bpp else 0
                line[x]=(line[x]+((a+prev[x])>>1))&255
        elif f==4:
            for x in range(stride):
                a=line[x-bpp] if x>=bpp else 0
                b=prev[x]; c=prev[x-bpp] if x>=bpp else 0
                pp=a+b-c; pa=abs(pp-a); pb=abs(pp-b); pc=abs(pp-c)
                pr=a if (pa<=pb and pa<=pc) else (b if pb<=pc else c)
                line[x]=(line[x]+pr)&255
        out+=line; prev=line
    return w,h,bpp,bytes(out)
a=read_png(sys.argv[1]); b=read_png(sys.argv[2])
if a[:3]!=b[:3]:
    print("GEOMETRY DIFFERS", a[:3], b[:3]); sys.exit(1)
w,h,bpp,pa=a; _,_,_,pb=b
n=0; first=None
for i in range(0,len(pa),bpp):
    if pa[i:i+bpp]!=pb[i:i+bpp]:
        n+=1
        if first is None:
            px=(i//bpp)%w; py=(i//bpp)//w
            first=(px,py,pa[i:i+bpp].hex(),pb[i:i+bpp].hex())
print("%s vs %s: %dx%d, %d of %d pixels differ" % (sys.argv[1].split('/')[-1], sys.argv[2].split('/')[-1], w, h, n, w*h))
if first: print("  first at x=%d y=%d  %s -> %s" % first)
