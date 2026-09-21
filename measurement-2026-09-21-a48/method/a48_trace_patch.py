#!/usr/bin/env python3
"""A48-TEMP (scratch copy only, AFTER kernel only): after a48's a79_patch.py,
append the recorded `gl_changerez`/`gl_nextrez` to the H02 `[aes] op` trace
line whenever the opcode is 121 (shel_write), so the serial log shows the
state Tier 1 records. The BEFORE kernel has no such accessors (HEAD bd9cd19)."""
import sys
p = sys.argv[1] + '/tos-api/src/aes/mod.rs'; s = open(p).read()
a = "            o[0], o[1], o[2], o[3], o[4], o[5], o[6], sw.is_some()));\n    }\n"
assert s.count(a) == 1, "trace anchor"
b = a + ("    // A48-TEMP: the recorded resolution-change request after shel_write.\n"
         "    if H02_TRACE.load(core::sync::atomic::Ordering::Relaxed) && pb.control(C_OPCODE) == 121 {\n"
         "        h02_log(format_args!(\"[a48] after shel_write: gl_changerez={} gl_nextrez={} sh_doexec={}\",\n"
         "            shell::gl_changerez(), shell::gl_nextrez(), shell::sh_doexec()));\n"
         "    }\n")
s = s.replace(a, b, 1)
open(p, 'w').write(s)
print("patched", p)
