#!/usr/bin/env python3
"""kfn.py <before.dis> <after.dis> [-v] -- DESK43 U5b: compare two kernels
function by function from `llvm-objdump -d --no-show-raw-insn` output.
Normalized: v0 crate disambiguators (Cs..._), legacy hashes, .llvm.N / .N
suffixes; addresses; branch targets kept as symbol+offset (placement-
independent); `adrp` pages and the :lo12: immediate of instructions using an
adrp register (data addresses). Prints functions only on one side, common
functions whose normalized bodies differ, and the result after applying the
known rename (RENAMES at the end)."""
import re,sys,difflib
def norm_sym(s):
    s=re.sub(r'Cs[0-9A-Za-z]+_','Cs_',s)
    s=re.sub(r'17h[0-9a-f]{16}E','17hE',s)
    s=re.sub(r'\.llvm\.\d+','',s)
    s=re.sub(r'\.\d+$','',s)
    return s
def load(p):
    funcs={}; cur=None; pagereg={}
    for l in open(p):
        l=l.rstrip('\n')
        m=re.match(r'^([0-9a-f]+) <(.*)>:$',l)
        if m:
            cur=norm_sym(m.group(2)); 
            k=cur; i=1
            while k in funcs: i+=1; k=f'{cur}#{i}'
            cur=k; funcs[cur]=[]; pagereg={}; continue
        if cur is None or not l.strip(): continue
        mm=re.match(r'^\s*[0-9a-f]+:\s*(.*)$',l)
        if not mm: continue
        ins=mm.group(1).strip()
        ins=re.sub(r'0x[0-9a-f]+ <([^>]*)>',lambda m:'<'+norm_sym(m.group(1))+'>',ins)
        op=ins.split()[0] if ins else ''
        if op=='adrp':
            r=ins.split()[1].rstrip(','); pagereg[r]=1
            ins='adrp '+r+' PAGE'
        else:
            regs=re.findall(r'\b([xw]\d+)\b',ins)
            # an instruction using a page register: normalize its #imm (lo12 offset)
            if any(('x'+x[1:]) in pagereg for x in regs[1:2]) or (op in('ldr','str','ldrb','ldrh','strb','strh','ldp','stp','add') and any(('x'+x[1:]) in pagereg for x in regs)):
                ins=re.sub(r'#0x[0-9a-f]+|#\d+','#LO12',ins)
            # destination overwrite clears page tracking
            if regs and op not in('str','strb','strh','stp','cmp','tst','cbz','cbnz','tbz','tbnz','b','bl','ret','br','blr'):
                d='x'+regs[0][1:]
                if d in pagereg and op!='add': pagereg.pop(d,None)
        funcs[cur].append(ins)
    return funcs
a=load(sys.argv[1]); b=load(sys.argv[2])
sa,sb=set(a),set(b)
print('functions',len(a),len(b),'only-before',len(sa-sb),'only-after',len(sb-sa))
for k in sorted(sa-sb): print('  - only before:',k[:150], len(a[k]))
for k in sorted(sb-sa): print('  + only after :',k[:150], len(b[k]))
diff=[k for k in sorted(sa&sb) if a[k]!=b[k]]
print('common functions with differing normalized bodies:',len(diff))
for k in diff:
    d=list(difflib.unified_diff(a[k],b[k],lineterm='',n=0))
    print('  ~',k[:150],'len',len(a[k]),len(b[k]),'difflines',len(d))
    if len(sys.argv)>3: print('\n'.join(d[:30]))

# Renamed functions (TedInfo::read_abi -> tedinfo_read_abi, DESK43 U5b): compare
# with the old name substituted by the new one, and re-check every differing
# common body under the same substitution.
RENAMES = {'_RNvMs1_NtNtCs_7tos_api3aes6objectNtB5_7TedInfo8read_abi': '_RNvNtNtCs_7tos_api3aes6object16tedinfo_read_abi'}
def sub(body):
    out = []
    for l in body:
        for o, n in RENAMES.items():
            l = l.replace(o, n)
        out.append(l)
    return out
for o, n in RENAMES.items():
    if o in a and n in b:
        print('renamed', o, '->', n, ': body identical after renaming' if sub(a[o]) == b[n] else ': BODY DIFFERS')
rest = [k for k in diff if sub(a[k]) != b[k]]
print('differing common bodies after applying the rename:', len(rest))
for k in rest:
    print('  ~', k)
