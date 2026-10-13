#!/usr/bin/env python3
"""uacc-instrumentation.py <scratch-tree> <bin-dir>

Applied AFTER measurement-2026-10-05-arch03-0c/method/apply-instrumentation.py
to the same scratch copy (never to the shared tree): on board-qemu-virt (no
SD card) the stage-3a probe UACC3A.NAP and its child UAKID.NAP join the RAM
program registry and UACC3A is launched from the boot path right after
CRASHTST. The crash prompts are answered over the serial line by drive.py
(--answer). Cases that need a C: drive are skipped by UACC3A itself.
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
progs = pathlib.Path(sys.argv[2]).resolve()
lib = t / "kernel/src/lib.rs"
s = lib.read_text()
old = '''        kprintln!("[c0c] back in the boot path after CRASHTST, launch rc={}", rc);
'''
assert s.count(old) == 1, "lib.rs CRASHTST anchor"
s = s.replace(old, old + f'''        // U3A-TEMP: the stage-3a bad-pointer probe
        static UAKID: &[u8] = include_bytes!("{progs}/UAKID.NAP");
        static UACC3A: &[u8] = include_bytes!("{progs}/UACC3A.NAP");
        tos_api::bdos::process::registry::register(b"C:\\\\UAKID.NAP", UAKID);
        tos_api::bdos::process::registry::register(b"C:\\\\UACC3A.NAP", UACC3A);
        let rc = launch::run_program_with_tail_rc(b"C:\\\\UACC3A.NAP", &[]);
        kprintln!("[u3a] back in the boot path after UACC3A, launch rc={{}}", rc);
''')
lib.write_text(s)
print("uacc instrumentation applied to", t)
