#!/usr/bin/env python3
"""squeeze.py <log> -- collapse runs of identical consecutive "[gh1] set_mown -> X" lines (the inline Control
Manager's w_setactive runs on every evnt_* pass and floods the log) into one line with the run length. In place;
every other line is kept byte-for-byte."""
import sys
p = sys.argv[1]; out = []; prev = None; n = 0
def flush():
    if prev is not None:
        out.append(prev if n == 1 else prev.rstrip("\n") + "  [x%d consecutive, collapsed by squeeze.py]\n" % n)
for line in open(p, errors="surrogateescape"):
    if line.startswith("[gh1] set_mown"):
        if line == prev: n += 1; continue
        flush(); prev, n = line, 1; continue
    flush(); prev, n = None, 0; out.append(line)
flush()
open(p, "w", errors="surrogateescape").writelines(out)
