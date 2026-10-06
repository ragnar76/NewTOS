#!/usr/bin/env python3
"""shotcmp.py <dirA> <prefixA> <dirB> <prefixB> -- ARCH03-4: byte-compare the
screendumps of two runs of the series: every <prefixA><rest>.png in dirA
against <prefixB><rest>.png in dirB. Prints SAME/DIFF/MISSING per file and a
count line. Differing files are then inspected by class (see RUNINFO)."""
import sys, os, hashlib
da, pa, db, pb = sys.argv[1:5]
def h(p):
    return hashlib.sha256(open(p, 'rb').read()).hexdigest()
same = diff = miss = 0
for f in sorted(os.listdir(da)):
    if not (f.startswith(pa) and f.endswith('.png')):
        continue
    rest = f[len(pa):]
    if pa.count('-') != pb.count('-') and rest.split('-')[0] in ('asid', 'tiny'):
        continue  # a longer mode name under the shorter prefix
    g = os.path.join(db, pb + rest)
    if not os.path.exists(g):
        print('MISSING', pb + rest); miss += 1; continue
    if h(os.path.join(da, f)) == h(g):
        same += 1
    else:
        print('DIFF', rest); diff += 1
print(f'{pa} vs {pb}: same={same} diff={diff} missing={miss}')
