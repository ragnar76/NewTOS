#!/usr/bin/env python3
"""hellogem-equiv.py <before.NAP> <after.NAP> -- DESK43 U5b: compare two builds of
HELLOGEM.NAP function by function (llvm-objdump). Crate-hash disambiguators
(`Cs..._`) and LLVM `.N` clone suffixes are normalized, branch targets are
compared as symbol+offset (placement-independent); clones of the same function
are compared as a multiset. Also prints the section sizes of both files."""
import re, subprocess, glob, os, sys, collections
SR = glob.glob(os.path.join(subprocess.run(['rustc','--print','sysroot'],capture_output=True,text=True).stdout.strip(),'lib/rustlib/*/bin'))[0]
def norm(s): return re.sub(r'\.\d+$', '', re.sub(r'Cs[0-9A-Za-z]+_', 'Cs_', s))
def target(m):
    t = re.sub(r'\.\d+(\+0x[0-9a-f]+)?$', lambda q: q.group(1) or '', m.group(1))
    return '<' + norm(t) + '>'
def load(p):
    d = subprocess.run([SR+'/llvm-objdump','-d','--no-show-raw-insn',p],capture_output=True,text=True).stdout
    f = collections.defaultdict(list); cur = None; n = 0
    for l in d.split('\n'):
        m = re.match(r'^([0-9a-f]+) <(.*)>:$', l.strip())
        if m: cur = []; f[norm(m.group(2))].append(cur); n += 1; continue
        mm = re.match(r'^\s*[0-9a-f]+:\s*(.*)$', l)
        if cur is not None and mm:
            cur.append(re.sub(r'0x[0-9a-f]+ <([^>]*)>', target, mm.group(1).strip()))
    return {k: sorted('\n'.join(x) for x in v) for k, v in f.items()}, n
(a, na), (b, nb) = load(sys.argv[1]), load(sys.argv[2])
print(f'functions {na} vs {nb}; distinct normalized names {len(a)} vs {len(b)}; same name set: {set(a) == set(b)}')
print('instructions', sum(len(x.split(chr(10))) for v in a.values() for x in v), 'vs', sum(len(x.split(chr(10))) for v in b.values() for x in v))
print('names whose bodies differ:', [k for k in a if a[k] != b.get(k)])
for p in sys.argv[1:3]:
    print(subprocess.run([SR+'/llvm-size','-A',p],capture_output=True,text=True).stdout)
