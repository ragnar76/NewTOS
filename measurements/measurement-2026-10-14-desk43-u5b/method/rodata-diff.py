#!/usr/bin/env python3
"""rodata-diff.py <before.elf> <after.elf> -- classify every differing u32 word of
.rodata / .data between two kernel ELFs: panic-Location line/column numbers
(resolved to the Location's file name) vs pointers shifted by a constant.
Needs llvm-objcopy/llvm-readobj from the Rust sysroot."""
import struct, collections, subprocess, sys, tempfile, os, glob
SR = glob.glob(os.path.join(subprocess.run(['rustc','--print','sysroot'],capture_output=True,text=True).stdout.strip(),'lib/rustlib/*/bin'))[0]
def sec(elf, name):
    with tempfile.NamedTemporaryFile(delete=False) as t: pass
    subprocess.run([SR+'/llvm-objcopy','-O','binary','--only-section='+name,elf,t.name],check=True)
    d=open(t.name,'rb').read(); os.unlink(t.name); return d
def addr(elf, name):
    out=subprocess.run([SR+'/llvm-readobj','-S',elf],capture_output=True,text=True).stdout.split('\n')
    for i,l in enumerate(out):
        if l.strip().startswith('Name: '+name+' ('):
            for k in out[i:i+12]:
                if 'Address:' in k: return int(k.split()[-1],16)
A,B=sys.argv[1],sys.argv[2]
for name in ('.data','.rodata'):
    a=sec(A,name); b=sec(B,name)
    print(f'{name}: size {len(a)} vs {len(b)}, differing bytes {sum(1 for i in range(min(len(a),len(b))) if a[i]!=b[i])}')
    if name!='.rodata' or len(a)!=len(b): continue
    base=addr(A,name)
    words=sorted(set((i//4)*4 for i in range(len(a)) if a[i]!=b[i]))
    kinds=collections.Counter(); files=collections.Counter()
    for w in words:
        x=struct.unpack_from('<I',a,w)[0]; y=struct.unpack_from('<I',b,w)[0]
        if x<20000 and y<20000:
            for off,kind in ((16,'line'),(20,'col')):
                if w-off<0: continue
                p=struct.unpack_from('<Q',a,w-off)[0]; n=struct.unpack_from('<Q',a,w-off+8)[0]
                if base<=p<base+len(a) and 0<n<200:
                    try: s=a[p-base:p-base+n].decode()
                    except UnicodeDecodeError: continue
                    if s.endswith('.rs'): files[(s,kind)]+=1; kinds['panic Location '+kind]+=1; break
            else: kinds['small int, Location not resolved']+=1
        else: kinds['pointer shifted by %#x'%(x-y)]+=1
    print('  differing u32 words:',len(words)); [print('   ',c,k) for k,c in kinds.items()]
    for (f,k),c in sorted(files.items()): print('    ',c,k,f)
