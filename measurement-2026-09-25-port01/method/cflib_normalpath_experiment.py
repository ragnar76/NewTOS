#!/usr/bin/env python3
"""PORT01 confirmation experiment (scratch copy only, NOT landed).

Hypothesis, read off the source: QED chops the last character of a file name
given on the command line because cflib's make_normalpath
(sdk/cflib/cflib-21/fsnorm.c:85-95) only leaves the trailing separator it
appended when the result is an EXISTING DIRECTORY, while QED removes that
separator unconditionally (src/poslist.c:53-54, src/dd.c:99-101).

The experiment makes make_normalpath keep the separator it appended -- the
behaviour QED is written against -- in a SCRATCH copy of cflib only, rebuilds
cflib.a and QED there, and re-runs the drop. If QED then opens the file, the
diagnosis is confirmed and the fix is located; nothing is landed either way.
"""
import sys

# The extracted cflib-21/ tree is a build artefact and is NOT watched for
# staleness (sdk/lib-stale.sh lib_inputs); the watched, applied-on-every-build
# channel is sdk/cflib/overlay/, which is also where a real fix would go.
src = sys.argv[1] + '/sdk/cflib/cflib-21/fsnorm.c'
p = sys.argv[1] + '/sdk/cflib/overlay/fsnorm.c'
s = open(src).read()
OLD = """	ret = path_exists(p);
	if (!ret)
		p[i] = '\\0';
"""
NEW = """	ret = path_exists(p);
	/* PORT01-EXPERIMENT (scratch only): keep the appended separator. */
"""
assert s.count(OLD) == 1, "fsnorm.c anchor not found"
open(p, 'w').write(s.replace(OLD, NEW, 1))
print("patched", p)
