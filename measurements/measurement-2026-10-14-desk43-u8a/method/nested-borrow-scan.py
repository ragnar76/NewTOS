#!/usr/bin/env python3
# nested-borrow-scan.py (run from the repo root) -- DESK43 U8a: heuristic static scan for a borrow of
# DESK (global::with / DESK.borrow) or SESSION (SESSION.borrow[_mut]) inside which a function is called
# that (transitively, by name) borrows the same cell again -- the case SpinMutex hung on and RefCell
# panics on. Name-based: a DeskState method and a free fn of the same name are not told apart.
import re, pathlib, sys
files = sorted(pathlib.Path('kernel/src/desk').glob('*.rs'))
src = {f.name: f.read_text() for f in files}
def strip_comments(s):
    s = re.sub(r'//[^\n]*', '', s)
    return s
# collect fn bodies
fns = {}  # name -> list of bodies
fnre = re.compile(r'\bfn\s+([a-z_][a-z0-9_]*)\s*(<[^>]*>)?\s*\(')
for name, s in src.items():
    s = strip_comments(s)
    for m in fnre.finditer(s):
        i = s.find('{', m.end())
        semi = s.find(';', m.end())
        if i < 0 or (0 <= semi < i): continue
        d = 0; j = i
        while j < len(s):
            if s[j] == '{': d += 1
            elif s[j] == '}':
                d -= 1
                if d == 0: break
            j += 1
        fns.setdefault(m.group(1), []).append((name, s[i:j+1]))
CELLS = {'DESK': r'global::with\(|\bwith\(\|g\||DESK\.borrow', 'SESSION': r'SESSION\.borrow'}
def direct(body, cell): return re.search(CELLS[cell], body) is not None
calls_re = re.compile(r'\b([a-z_][a-z0-9_]*)\s*\(')
# transitive "borrows cell" set
for cell in CELLS:
    borrowers = {n for n, bl in fns.items() if any(direct(b, cell) for _, b in bl)}
    changed = True
    while changed:
        changed = False
        for n, bl in fns.items():
            if n in borrowers: continue
            for _, b in bl:
                if any(c in borrowers for c in calls_re.findall(b)):
                    borrowers.add(n); changed = True; break
    # now find borrow regions and calls inside them
    pat = {'DESK': r'global::with\(|(?<![:\w])with\(\|', 'SESSION': r'SESSION\.borrow(_mut)?\(\)'}[cell]
    for name, s in src.items():
        s2 = strip_comments(s)
        for m in re.finditer(pat, s2):
            if cell == 'DESK':
                # region = the with( ... ) argument
                i = m.end() - 1; d = 0; j = i
                while j < len(s2):
                    if s2[j] == '(': d += 1
                    elif s2[j] == ')':
                        d -= 1
                        if d == 0: break
                    j += 1
                region = s2[m.end():j]
            else:
                # guard lives to end of enclosing block if bound with let, else statement
                line_start = s2.rfind('\n', 0, m.start())
                stmt = s2[line_start:m.start()]
                if 'let ' in stmt:
                    # to end of enclosing block
                    d = 0; j = m.end()
                    while j < len(s2):
                        if s2[j] == '{': d += 1
                        elif s2[j] == '}':
                            if d == 0: break
                            d -= 1
                        j += 1
                    region = s2[m.end():j]
                    # drop(guard) shortens it
                    k = region.find('drop(guard)')
                    if k >= 0: region = region[:k]
                else:
                    region = s2[m.end():s2.find(';', m.end())]
            hits = sorted({c for c in calls_re.findall(region) if c in borrowers and c not in ('with',)})
            if cell=='DESK': hits=[h for h in hits]
            if hits:
                ln = s2[:m.start()].count('\n') + 1
                print(f"{cell} {name}:{ln}: inside borrow calls {hits}")
