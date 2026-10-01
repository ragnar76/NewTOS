#!/usr/bin/env python3
"""ACC diagnosis (scratch copy only): add the AES *identity* and the raw
control words to the H02 opcode trace, so a traced call can be attributed to a
client. `appl::current_pid()` is 0 for every application in the chain
(A56/`reported_pid`), which is exactly what made the op-19 flood in the first
acc2 run unattributable."""
import sys
p = sys.argv[1] + '/tos-api/src/aes/mod.rs'; s = open(p).read()
a = 'h02_log(format_args!("[aes] pid={} op {}'
assert s.count(a) == 1
s = s.replace('h02_log(format_args!("[aes] pid={} op {} in=({},{},{},{},{},{}) out=({},{},{},{},{},{},{}) yield={}",\n'
              '            appl::current_pid(), op,',
              'h02_log(format_args!("[aes] pid={} id={} ctl=({},{},{},{}) op {} in=({},{},{},{},{},{}) out=({},{},{},{},{},{},{}) yield={}",\n'
              '            appl::current_pid(), appl::current_identity(),\n'
              '            pb.control(0), pb.control(1), pb.control(2), pb.control(3), op,', 1)
open(p, 'w').write(s)
print("patched", p)
