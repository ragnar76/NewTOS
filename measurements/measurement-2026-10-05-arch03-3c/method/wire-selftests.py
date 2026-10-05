#!/usr/bin/env python3
"""wire-selftests.py <scratch-tree>

Scratch-only: call the nine kernel selftests whose hand-built ELF blobs 0d
changed (ET_EXEC -> ET_DYN; selftest_pexec/_d4d5 moved from VA 0x100 to
0x1000) from the boot path, right before the AUTO folder. They are not wired
in the product boot (memory note: unwired selftests rot silently).
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
lib = t / "kernel/src/lib.rs"
s = lib.read_text()
old = "    launch::run_auto_folder();\n"
assert s.count(old) == 1
calls = [
    "selftest_d3::run_d3_elf_loader_test()",
    "selftest_d6::run_d6_pterm_test()",
    "selftest_d7::run_d7_ptermres_test()",
    "selftest_pexec::run_pexec_test()",
    "selftest_pexec::run_pexec_env_test()",
    "selftest_pexec_d4d5::run_pexec_loadgo_split_test()",
    "selftest_sysvars::run()",
    "selftest_upcall::run()",
    "selftest_etv_term::run()",
    "selftest_sleepwake_switch::run()",
]
body = "".join(f'    kprintln!("[d0d-st] >>> {c}");\n    {c};\n' for c in calls)
s = s.replace(old, "    // D0D-TEMP BEGIN\n" + body + '    kprintln!("[d0d-st] all selftests returned");\n    // D0D-TEMP END\n' + old)
lib.write_text(s)
print("wired", len(calls), "selftests into", lib)
