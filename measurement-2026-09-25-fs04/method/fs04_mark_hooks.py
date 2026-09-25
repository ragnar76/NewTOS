#!/usr/bin/env python3
"""FS04-TEMP: let the injector's MARK steps drive the probe.

MARK "RESET"  -> zero every counter and start the wall clock
MARK "REPORT" -> print the distribution accumulated since RESET

Applied to the scratch copy AFTER apply_injector.py.
"""
import sys
scr = sys.argv[1]
p = scr + '/kernel/src/host_env.rs'
s = open(p).read()
old = """                Step::Mark(_) | Step::Pause => {}"""
new = """                Step::Mark(m) => {
                    // FS04-TEMP: the two marks that drive the probe.
                    if m == "RESET" {
                        crate::desk::fs04drv::mark_reset();
                    } else if m == "REPORT" {
                        crate::desk::fs04drv::mark_report("gesture");
                    }
                }
                Step::Pause => {}"""
assert s.count(old) == 1, s.count(old)
open(p, 'w').write(s.replace(old, new, 1))
print("fs04 mark hooks applied")
