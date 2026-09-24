#!/usr/bin/env python3
"""Undo the FS01 fix in a SCRATCH COPY of the tree (never the shared tree).

`dir::erase_slot_with_lfn` returns right after it has marked the 8.3 entry
erased, so the long-name slots in front of it stay live -- exactly the
behaviour `Fdelete`/`Ddelete` had before FS01. Everything else (call sites,
signatures) is unchanged, so the "before" and "after" kernels differ in this
one branch only.
"""
import sys

p = sys.argv[1] + '/tos-api/src/bdos/fs/dir.rs'
s = open(p).read()
a = "    // Then its long-name chain, ordinal 1 upwards, walking backwards.\n"
b = ("    // FS01-UNFIX (scratch copy only): the pre-FS01 behaviour -- the 8.3\n"
     "    // entry is freed and its long-name slots are left behind.\n"
     "    if true {\n"
     "        return true;\n"
     "    }\n")
assert s.count(a) == 1, "anchor not found"
s = s.replace(a, b + a, 1)
open(p, 'w').write(s)
print("unfixed", p)
